#!/usr/bin/env node
// Checks Preku skill files the way the app does before installing them.
// No dependencies: needs Node 18 or newer.
//
//   node skills/tools/validate.mjs my-skill.json [more.json …]
//   node skills/tools/validate.mjs --name-is-id skills/community/*.json   (as CI does)
//
// --name-is-id also requires each file to be named after its id (<id>.json), as shared
// skills in skills/community are.
// Exit code 0 when every file is valid, 1 otherwise. The app's own checker
// (desktop/src-tauri/src/modules/skills/format.rs) is the reference; keep the two in step.

import { readFileSync, statSync } from "node:fs";
import { basename } from "node:path";

const MAX_FILE_BYTES = 256 * 1024;
const SLUG = /^[a-z0-9][a-z0-9-]{0,31}$/;
const CLOCK = /^([01]\d|2[0-3]):([0-5]\d)$/;
const TOP = ["$schema", "formatVersion", "id", "name", "version", "author", "description", "license", "homepage", "contributes"];

function check(skill) {
  const out = [];
  const isObject = (v) => v !== null && typeof v === "object" && !Array.isArray(v);
  const unknown = (where, value, allowed) => {
    if (!isObject(value)) return out.push(`${where} must be an object`), true;
    for (const key of Object.keys(value))
      if (!allowed.includes(key)) out.push(`${where}: unknown field "${key}"`);
    return false;
  };
  const text = (field, value, max) => {
    if (typeof value !== "string") return out.push(`${field} must be text`);
    const length = [...value.trim()].length;
    if (length === 0) out.push(`${field} is empty`);
    else if (length > max) out.push(`${field} is ${length} characters; the limit is ${max}`);
    // eslint-disable-next-line no-control-regex
    if (/[\u0000-\u001f\u007f-\u009f]/.test(value)) out.push(`${field} contains control characters (line breaks, tabs…)`);
  };
  const https = (field, value) => {
    if (value === undefined) return;
    let ok = false;
    try {
      const url = new URL(value);
      ok = url.protocol === "https:" && !!url.hostname && value.length <= 300;
    } catch {}
    if (!ok) out.push(`${field} must be an https:// address`);
  };
  const list = (field, value) => {
    if (value === undefined) return [];
    if (!Array.isArray(value)) return out.push(`${field} must be a list`), [];
    return value;
  };

  if (unknown("the skill", skill, TOP)) return out;
  if (skill.formatVersion !== 1) out.push(`formatVersion is ${skill.formatVersion}; this Preku reads version 1`);
  const parts = typeof skill.id === "string" ? skill.id.split(".") : [];
  if (!(parts.length >= 2 && parts.length <= 6 && skill.id.length <= 100 && parts.every((p) => SLUG.test(p))))
    out.push("id must look like io.github.yourname.skill-name (lowercase letters, digits, dashes, dots)");
  text("name", skill.name, 40);
  text("description", skill.description, 200);
  if (!unknown("author", skill.author, ["name", "url"])) {
    text("author.name", skill.author.name, 60);
    https("author.url", skill.author.url);
  }
  if (!(typeof skill.version === "string" && skill.version.length <= 40 && /^\d{1,6}\.\d{1,6}\.\d{1,6}([-+].*)?$/.test(skill.version)))
    out.push("version must be a semantic version such as 1.0.0");
  https("homepage", skill.homepage);
  if (skill.license !== undefined) text("license", skill.license, 40);

  if (unknown("contributes", skill.contributes, ["thoughts", "nudges", "sounds"])) return out;
  const thoughts = list("thoughts", skill.contributes.thoughts);
  const nudges = list("nudges", skill.contributes.nudges);
  const sounds = list("sounds", skill.contributes.sounds);
  if (!thoughts.length && !nudges.length && !sounds.length) out.push("contributes is empty: add thoughts, nudges or sounds");

  if (thoughts.length > 500) out.push(`${thoughts.length} thoughts; the limit is 500`);
  thoughts.forEach((t, i) => {
    if (unknown(`thoughts[${i}]`, t, ["text", "author"])) return;
    text(`thoughts[${i}].text`, t.text, 140);
    if (t.author !== undefined) text(`thoughts[${i}].author`, t.author, 60);
  });

  if (sounds.length > 5) out.push(`${sounds.length} sounds; the limit is 5`);
  const soundIds = new Set();
  sounds.forEach((s, i) => {
    if (unknown(`sounds[${i}]`, s, ["id", "name", "notes"])) return;
    if (!SLUG.test(s.id ?? "") || s.id === "default" || s.id === "none")
      out.push(`sounds[${i}].id must be a short slug (not "default" or "none")`);
    if (soundIds.has(s.id)) out.push(`sounds[${i}].id "${s.id}" is used twice`);
    soundIds.add(s.id);
    text(`sounds[${i}].name`, s.name, 30);
    const notes = list(`sounds[${i}].notes`, s.notes);
    if (notes.length === 0 || notes.length > 12) out.push(`sounds[${i}] needs 1–12 notes`);
    notes.forEach((n, j) => {
      const at = `sounds[${i}].notes[${j}]`;
      if (unknown(at, n, ["freq", "at", "length", "gain", "wave"])) return;
      const nums = [n.freq, n.at, n.length, n.gain];
      if (
        !nums.every((v) => typeof v === "number" && Number.isFinite(v)) ||
        n.freq < 50 || n.freq > 4000 || n.at < 0 || n.at > 2 ||
        n.length < 0.01 || n.length > 1.5 || n.gain < 0 || n.gain > 0.4
      )
        out.push(`${at}: freq 50–4000 Hz, at 0–2 s, length 0.01–1.5 s, gain 0–0.4`);
      else if (n.at + n.length > 2) out.push(`${at} ends after 2 s; sounds must stay short`);
      if (n.wave !== undefined && !["sine", "triangle", "square"].includes(n.wave))
        out.push(`${at}.wave must be sine, triangle or square`);
    });
  });

  if (nudges.length > 10) out.push(`${nudges.length} nudges; the limit is 10`);
  const nudgeIds = new Set();
  nudges.forEach((n, i) => {
    const at = `nudges[${i}]`;
    if (unknown(at, n, ["id", "lines", "everyMin", "random", "days", "between", "sound"])) return;
    if (!SLUG.test(n.id ?? "")) out.push(`${at}.id must be a short slug (a-z, 0-9, -)`);
    if (nudgeIds.has(n.id)) out.push(`${at}.id "${n.id}" is used twice`);
    nudgeIds.add(n.id);
    const lines = list(`${at}.lines`, n.lines);
    if (lines.length === 0 || lines.length > 20) out.push(`${at} needs 1–20 lines`);
    lines.forEach((line, j) => text(`${at}.lines[${j}]`, line, 60));
    if (!(Number.isInteger(n.everyMin) && n.everyMin >= 5 && n.everyMin <= 240))
      out.push(`${at}.everyMin must be 5–240 minutes`);
    if (n.random !== undefined && typeof n.random !== "boolean") out.push(`${at}.random must be true or false`);
    if (n.days !== undefined && !(Array.isArray(n.days) && n.days.length > 0 && n.days.every((d) => Number.isInteger(d) && d >= 1 && d <= 7)))
      out.push(`${at}.days lists weekdays 1 (Monday) to 7 (Sunday)`);
    if (n.between !== undefined) {
      const ok = Array.isArray(n.between) && n.between.length === 2 && n.between.every((t) => CLOCK.test(t)) && n.between[0] !== n.between[1];
      if (!ok) out.push(`${at}.between must be two different times like ["09:00", "18:00"]`);
    }
    if (n.sound !== undefined && n.sound !== "default" && n.sound !== "none" && !soundIds.has(n.sound))
      out.push(`${at}.sound "${n.sound}" is not one of this skill's sounds`);
  });
  return out;
}

const nameIsId = process.argv.includes("--name-is-id");
const files = process.argv.slice(2).filter((arg) => arg !== "--name-is-id");
if (files.length === 0) {
  console.error("Usage: node validate.mjs <skill.json> [more.json …]");
  process.exit(2);
}
let failed = false;
for (const file of files) {
  let problems;
  try {
    if (statSync(file).size > MAX_FILE_BYTES) problems = ["the file is larger than 256 KB"];
    else {
      const raw = readFileSync(file, "utf8").replace(/^﻿/, "");
      const skill = JSON.parse(raw);
      problems = check(skill);
      if (nameIsId && basename(file) !== `${skill?.id}.json`)
        problems.push(`the file should be named ${skill?.id}.json`);
    }
  } catch (err) {
    problems = [`not a valid skill file: ${err.message}`];
  }
  if (problems.length === 0) {
    console.log(`✓ ${file}`);
  } else {
    failed = true;
    console.log(`✗ ${file}`);
    for (const problem of problems) console.log(`    ${problem}`);
  }
}
process.exit(failed ? 1 : 0);
