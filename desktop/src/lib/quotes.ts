// "Thought for today": one short line Pip says when Preku starts. Written for Preku (no
// attributions to get wrong, nothing fetched), in Pip's voice: kind, practical, never preachy,
// and in line with the wellbeing wording rules (no guilt, no health claims).
// Shown large on the "A moment for you" card: one or two short sentences, at most 90 characters.

export const THOUGHTS = [
  "You don't need to finish everything today. Start with what matters.",
  "One small task done is still a task done.",
  "Start with the easy one. Momentum counts.",
  "You don't have to finish it. Just begin it.",
  "A short break now is a clearer head later.",
  "Slow is fine. Stopped is fine too, for a while.",
  "Pick three things. The rest can wait its turn.",
  "Done is a very good colour on a to-do list.",
  "Water first, then the hard thing.",
  "Twenty-five focused minutes go a long way.",
  "It's okay to close a few tabs.",
  "The best plan is the one you actually start.",
  "Rest is part of the work, not a pause from it.",
  "Be as patient with yourself as with a friend.",
  "Little steps still move you forward.",
  "Look out of a window for a moment. It helps.",
  "What would make today feel like enough?",
  "You've handled every hard day so far.",
  "Unclench your jaw. Drop your shoulders. Hi.",
  "One thing at a time is still the fastest way.",
  "A messy first draft beats a perfect blank page.",
  "Say no to one thing so you can say yes to another.",
  "Finish something small before lunch.",
  "Your pace is allowed to be your pace.",
  "Tidy desk, tidy five minutes. Worth a try.",
  "Ask for help early. It's a shortcut, not a detour.",
  "Good enough, sent today, beats perfect, sent never.",
  "Stretch like a cat that just woke up.",
  "Today's list is a menu, not a contract.",
  "Breathe out slowly. Now, where were we?",
  "The hard part is usually the first two minutes.",
  "Celebrate the boring wins. They add up.",
  "Leave a note for tomorrow-you. They'll thank you.",
  "It's fine to change your mind about the plan.",
  "Stand up, look far away, come back. Easy.",
  "Do the thing you keep moving to tomorrow.",
  "You're allowed to enjoy the work.",
  "Curiosity gets more done than pressure.",
  "A walk solves more bugs than staring does.",
  "Protect one quiet hour today if you can.",
  "End the day by writing down one thing that went well.",
] as const;

/** Day number since 1970 in local time, so the thought changes at local midnight. */
export function localDay(date: Date): number {
  return Math.floor((date.getTime() - date.getTimezoneOffset() * 60_000) / 86_400_000);
}

/** The thought for this local day: the same all day, a different one tomorrow. */
export function thoughtFor(date: Date): string {
  const index = ((localDay(date) % THOUGHTS.length) + THOUGHTS.length) % THOUGHTS.length;
  return THOUGHTS[index]!;
}
