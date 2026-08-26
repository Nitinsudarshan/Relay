# Rule: Meeting Titles & Headings

**Applies to:** any agent or local model generating or repairing the display title of a recorded meeting.
**Output:** one line of plain text. No quotes, no Markdown, no explanation, no alternatives.

---

## 1. Why this rule exists

Impromptu recordings are created with no user-supplied name, so the app writes a placeholder such as `Meeting -Aug 26, 2026 02:03PM`. That string is a timestamp, not a title — it tells the user nothing when they scan a list of thirty recordings.

The naive fix — "use the first line of the transcript" — fails badly, because the opening seconds of a recording are the least reliable part of it. This rule exists to produce a title from **what the meeting was about**, not from **what was said first**.

---

## 2. When to generate a title

Generate a new title when the current title matches any placeholder pattern:

- `Meeting - <date>` / `Meeting -<date>` / `Meeting <date>` (any spacing)
- `New Recording`, `Untitled`, `Untitled Meeting`, `Recording <n>`
- A bare timestamp, date, or filename (`2026-08-26_1403.wav`, `rec_004`)
- Empty or whitespace only
- A title that begins with a bracketed ASR tag (`[no audio]…`, `[inaudible]…`) — this indicates a previous bad auto-generation and must be regenerated

**Do not** overwrite a title the user typed themselves, or a title carried in from a calendar invite. If the title contains no date/timestamp and reads as intentional prose, leave it exactly as it is.

---

## 3. Construction rules

1. **3–8 words.** Under 60 characters. It must survive truncation in a narrow sidebar.
2. **Topic first.** The dominant subject of the meeting is the first thing in the title.
3. **Title Case** for significant words.
4. **No terminal punctuation.** No period, no ellipsis, no trailing comma or dash.
5. **No dates or times.** The UI already shows those. Include a date only when the meeting is *about* a date ("Q3 Launch Date Review").
6. **No speaker names** unless the meeting is defined by the person — a 1:1, an interview, a candidate review ("1:1 with Rahul", "Candidate Debrief — Anita Rao").
7. **No hedging.** Never "Possible Discussion About…", "Meeting Regarding…", "General Chat On…".
8. **No filler nouns.** Drop "Meeting", "Call", "Sync", "Discussion" unless the format is the point ("Weekly Standup", "Sprint Retro").
9. **Prefer nouns over verbs.** "Onboarding Drop-off Investigation" beats "Investigating Onboarding Drop-off".
10. **Use the meeting's own vocabulary.** If participants consistently say "Phone Manager", the title says Phone Manager — not "Mobile Application".

---

## 4. Absolute prohibitions

These are the failure modes this rule was written to stop:

1. **Never copy the opening span of the transcript.** Not the first line, not the first sentence, not the first N words. The opening is where cold audio, joining noise, background TV, and mid-thought fragments live.
2. **Never emit a bracketed ASR tag.** `[no audio]`, `[inaudible]`, `[BLANK_AUDIO]`, `[music]`, `[laughter]`, `(unintelligible)` are stripped before you begin reasoning and can never reach the output.
3. **Never truncate a phrase to fit.** A title that ends mid-list or mid-clause ("Clean Assistant, Phone Clone and MIMO") is worse than a generic fallback. If your candidate ends on `and`, `or`, `the`, `to`, `of`, `with`, `for`, or a comma, it is truncated — discard it.
4. **Never title from a phrase that appears exactly once.** A one-off phrase is usually an ASR error or a passing aside.
5. **Never invent a subject.** If the transcript never establishes a topic, use the fallback ladder in Section 7 rather than guessing one.
6. **Never output more than one title.** No "Option A / Option B".

---

## 5. Procedure

Follow these steps in order.

### Step 1 — Clean

Remove every bracketed or parenthesized tag, every timestamp, and every speaker label. Work only on the remaining speech.

### Step 2 — Skip the cold open

Discard the first ~200 words (or the first 30 seconds if timestamps exist), **unless** the transcript is shorter than 400 words total, in which case discard only leading fragments that are not complete sentences.

### Step 3 — Sample the whole recording

Read the beginning (post-skip), the middle, and the end. A meeting's real subject is usually stated in the middle, and confirmed at the end when someone recaps.

### Step 4 — Find the recurring subject

Identify the noun phrase or project name that appears most often across at least two of the three regions. That is the topic. Proper nouns and product names outrank generic nouns.

### Step 5 — Identify the intent

Pick the verb-sense of the meeting: review, decision, planning, debugging, naming, kickoff, retro, demo, interview, status. Combine it with the topic.

### Step 6 — Verify

Run the checklist in Section 9. If any check fails, go to the fallback ladder.

---

## 6. Worked example — the known failure case

**Transcript opening (verbatim ASR):**

> [no audio] Clean Assistant, Phone Clone and MIMO Bar Butler is called Phone Manager. presumably because…

**Previously generated title (wrong):**

```
[no audio] Clean Assistant, Phone Clone and  MIMO
```

Three separate rule violations:

| Violation | Rule |
| --- | --- |
| Kept the `[no audio]` tag | §4.2 |
| Copied the opening span verbatim | §4.1 |
| Truncated mid-list, ending on "and" | §4.3 |

The result is also semantically wrong: "Clean Assistant" and "Phone Clone" are items being *listed in passing*, and the actual subject of the sentence is **Phone Manager**. Had the model sampled the middle of the transcript, where the participants discuss the app's features and naming, the topic would have been unambiguous.

**Correct title:**

```
Phone Manager App Naming Review
```

If the rest of the transcript turns out to be about bundled system apps generally rather than naming, then:

```
Preinstalled Phone Utilities Walkthrough
```

Either is acceptable. `[no audio] Clean Assistant…` is not.

---

## 7. Fallback ladder

Use the first rung that applies. Never skip straight to the bottom.

1. **Topic is clear** → `<Topic> <Intent>` — e.g. `OTP Drop-off Investigation`
2. **Two participants, conversational, no single topic** → `1:1 with <Name>` (only if the name is certain)
3. **Recurring format is recognizable but topic is diffuse** → `Weekly Standup`, `Sprint Retro`, `Design Review`
4. **Some intelligible speech, no discernible topic** → `Untitled Meeting — <D MMM>` e.g. `Untitled Meeting — 26 Aug`
5. **Under ~100 intelligible words, or mostly silence/artifacts** → `Short Recording — <D MMM>`
6. **Nothing intelligible at all** → keep the original placeholder unchanged and mark the record as `title_generated: false`

Rungs 4–6 are the only places a date is permitted in a title.

---

## 8. More examples

| Transcript gist | Bad title | Good title |
| --- | --- | --- |
| Debugging why Android OTP screen loses users | `so um the reason I pulled everyone in` | `Android OTP Drop-off Debugging` |
| Vendor demo of an invoicing tool | `[inaudible] thanks for joining everyone` | `Invoicing Tool Vendor Demo` |
| Weekly team status, five topics, no focus | `Various Updates and Some Other Things` | `Weekly Standup` |
| Two people planning a Goa trip | `Meeting - Aug 26, 2026 02:03PM` | `Goa Trip Planning` |
| Interview with a backend candidate | `Discussion Regarding the Candidate` | `Backend Interview — Anita Rao` |
| 40 seconds of keyboard noise and one cough | `Typing and Ambient Sound` | `Short Recording — 26 Aug` |

---

## 9. Self-check before returning

- [ ] Output is a single line, unquoted, with no Markdown and no trailing punctuation.
- [ ] Between 3 and 8 words, under 60 characters.
- [ ] Contains no `[`, `]`, `(`, `)`, or any ASR tag.
- [ ] Does not reuse the opening words of the transcript.
- [ ] Does not end on a conjunction, article, preposition, or comma.
- [ ] The subject in the title appears at least twice in the transcript.
- [ ] Contains no date unless fallback rung 4 or 5 was used.
- [ ] A person who did not attend could tell from this title what the meeting was about.

If any box fails, drop one rung on the fallback ladder and re-check.

---

## 10. Model settings

- Temperature `0.3`. Slightly above extraction, since phrasing matters, but low enough to stay grounded.
- Cap output at ~20 tokens and stop on newline — this makes truncation-style failures impossible to emit as a full response.
- Do **not** prefill with any transcript text; prefilling with the opening line is what produces §4.1 failures.
