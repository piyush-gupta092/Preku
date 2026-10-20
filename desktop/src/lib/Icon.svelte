<script lang="ts">
  // Outline icons drawn as single paths on a 24×24 grid (Design.md §4: 1.75 stroke, local SVG).
  let { name, size = 20 }: { name: string; size?: number } = $props();

  const paths: Record<string, string> = {
    productivity: "M12 8v4l3 2 M9 2h6 M12 2v3 M19 5l-2 2 M21 13a9 9 0 1 1-18 0 9 9 0 0 1 18 0",
    monitor: "M3 12h4l3-7 4 14 3-7h4",
    wellbeing: "M20 4C9 2 3 8 6 16c8 4 15-2 14-12Z M6 20l9-10",
    bridge: "M8 2h8a2 2 0 0 1 2 2v16a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2Z M10 18h4",
    home: "M3 10.5 12 3l9 7.5 M5 9v11h5v-6h4v6h5V9",
    focus: "M12 8v4l3 2 M9 2h6 M12 2v3 M21 13a9 9 0 1 1-18 0 9 9 0 0 1 18 0",
    tasks: "M9 11l3 3 8-8 M20 12v7a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h9",
    clipboard:
      "M9 3h6a1 1 0 0 1 1 1v2H8V4a1 1 0 0 1 1-1Z M8 5H6a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7a2 2 0 0 0-2-2h-2 M8 12h8 M8 16h5",
    play: "M7 4v16l13-8Z",
    pause: "M7 4h3v16H7Z M14 4h3v16h-3Z",
    stop: "M6 6h12v12H6Z",
    next: "M5 4v16l10-8Z M19 4v16",
    previous: "M19 4v16L9 12Z M5 4v16",
    pin: "M12 17v5 M9 3h6l-1 6 4 4v2H6v-2l4-4Z",
    trash: "M4 7h16 M10 11v6 M14 11v6 M6 7l1 13h10l1-13 M9 7V4h6v3",
    up: "M6 15l6-6 6 6",
    down: "M6 9l6 6 6-6",
    plus: "M12 5v14 M5 12h14",
    search: "M11 4a7 7 0 1 1 0 14 7 7 0 0 1 0-14Z M20 20l-4-4",
    close: "M6 6l12 12 M18 6 6 18",
    check: "M5 12l5 5 9-10",
    back: "M15 6l-6 6 6 6",
    settings:
      "M12 9a3 3 0 1 1 0 6 3 3 0 0 1 0-6Z M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1Z",
    gauge: "M12 14l4-4 M3.3 19a9 9 0 1 1 17.4 0Z",
    list: "M8 6h13 M8 12h13 M8 18h13 M3.5 6h.01 M3.5 12h.01 M3.5 18h.01",
    code: "M16 18l6-6-6-6 M8 6l-6 6 6 6",
    bell: "M6 8a6 6 0 1 1 12 0c0 7 3 9 3 9H3s3-2 3-9 M10.3 21a1.9 1.9 0 0 0 3.4 0",
    restart: "M3 12a9 9 0 1 0 3-6.7L3 8 M3 3v5h5",
    globe:
      "M12 3a9 9 0 1 1 0 18 9 9 0 0 1 0-18Z M3 12h18 M12 3a14 14 0 0 1 0 18 M12 3a14 14 0 0 0 0 18",
    box: "M21 8l-9-5-9 5v8l9 5 9-5Z M3 8l9 5 9-5 M12 13v8",
    terminal: "M4 17l6-6-6-6 M12 19h8",
    git: "M6 3v12 M18 9a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z M6 21a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z M18 9a9 9 0 0 1-9 9",
    notch:
      "M3 6a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z M8 4v2a2 2 0 0 0 2 2h4a2 2 0 0 0 2-2V4",
    clock: "M12 3a9 9 0 1 1 0 18 9 9 0 0 1 0-18Z M12 7v5l3 2",
    grid: "M4 4h6v6H4Z M14 4h6v6h-6Z M4 14h6v6H4Z M14 14h6v6h-6Z",
    info: "M12 3a9 9 0 1 1 0 18 9 9 0 0 1 0-18Z M12 11v5 M12 8h.01",
    weather:
      "M12 2v2 M4.9 4.9l1.4 1.4 M2 12h2 M7.5 11a4.5 4.5 0 0 1 8.4-2.2 M9 21h8a4 4 0 0 0 0-8 5.5 5.5 0 0 0-10.4 2.2A3 3 0 0 0 9 21Z",
    demo: "M12 3l2 6 6 2-6 2-2 6-2-6-6-2 6-2Z",
    skills:
      "M10 4a2 2 0 1 1 4 0v2h4v4h-2a2 2 0 1 0 0 4h2v4h-4v-2a2 2 0 1 0-4 0v2H6v-4h2a2 2 0 1 0 0-4H6V6h4Z",
    music: "M9 18V5l11-2v13 M9 18a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z M20 16a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z",
  };
</script>

<svg
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.75"
  stroke-linecap="round"
  stroke-linejoin="round"
  style:width="{size}px"
  style:height="{size}px"
  aria-hidden="true"
>
  <path d={paths[name] ?? "M5 12h14 M12 5v14"} />
</svg>

<style>
  svg {
    flex: none;
  }
</style>
