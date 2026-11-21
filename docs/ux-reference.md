# Grok Bot UX reference

Research date: 2026-11-21.
Product: Grok Bot by xAI (the company now shows the name "SpaceX Super Intelligence LLC" in its page footer).
Purpose: a reference for the layout and interaction patterns of the Grok Bot desktop app. We use it to design the Hermes Bots desktop app.

## How to read this document

- **Confirmed** means that an official xAI page (news, docs, or changelog) states the fact.
- **Reported** means that only a third-party review, guide, or clone project states the fact.
- **Not confirmed** means that no source states the fact. In that case, we give a design that fits the known patterns.
- Text in quotation marks is a UI label or a short phrase that a source shows. We did not invent quotes.
- We did not copy images or logos.

Important fact: the marketing text says "each Bot has its own computer". The docs say that all Bots of one account share one cloud computer. Each Bot gets its own screen on that computer. The docs say that screens are "separate work surfaces, not separate security boundaries".

---

## 1. Sources

### Official xAI sources

| # | Title | URL | Date |
|---|---|---|---|
| 1 | Introducing Grok Bot | https://x.ai/news/introducing-grok-bot | 2026-08-11 |
| 2 | Designing Grok Bot for a world of persistent agents | https://x.ai/news/designing-grok-bot | 2026-09-03 |
| 3 | Grok Bot 1.0 (launch post) | https://x.ai/news/grok-bot-1-0 | 2026-11-11 |
| 4 | Grok Bot product page ("Meet Grok Bot") | https://x.ai/bot | Live page, read 2026-11-21 |
| 5 | Grok Bot Changelog | https://x.ai/changelog/bot | Live page (blocked to our fetch tool; read through mirror #18) |
| 6 | Docs: Bots | https://docs.x.ai/grok-bot/bots | Live page |
| 7 | Docs: Chat and collaboration (includes keyboard shortcuts) | https://docs.x.ai/grok-bot/chat-and-collaboration | Live page |
| 8 | Docs: Computer and apps | https://docs.x.ai/grok-bot/computer-and-apps | Live page |
| 9 | Docs: Vault and secure sign-in | https://docs.x.ai/grok-bot/vault | Live page |
| 10 | Docs: Settings and notifications | https://docs.x.ai/grok-bot/settings-and-notifications | Live page |
| 11 | Docs: Approvals, security and privacy | https://docs.x.ai/grok-bot/approvals-security-and-privacy | Live page |
| 12 | Docs: Troubleshooting | https://docs.x.ai/grok-bot/troubleshooting | Live page |
| 13 | Docs: Mobile | https://docs.x.ai/grok-bot/mobile | Live page |

### Third-party sources

| # | Title | URL | Date |
|---|---|---|---|
| 14 | How to Set Up Grok Bot and Build Your First AI Agents (MindStudio) | https://www.mindstudio.ai/blog/grok-bot-setup-guide | 2026-08-12 |
| 15 | What Is Grok Bot? A Paying User's Week-One Review (C. Alarcon) | https://christopheralarcon.com/blog/what-is-grok-bot | 2026-08-28 (fix 2026-11-13) |
| 16 | What Can Grok Bot Do? 7 AI Agent Ideas I Run From My Phone (The AI Maker) | https://aimaker.substack.com/p/grok-bot-ai-agent-team | 2026-09-06 |
| 17 | Hand Off Real Work Across Your Apps With Grok Bot (The Rundown) | https://app.therundown.ai/guides/hand-off-real-work-across-your-apps-with-grok-bot | 2026-09-11 |
| 18 | Grok Bot Changelog (November 2026) (gradually.ai mirror) | https://www.gradually.ai/en/changelogs/grok-bot/ | Updated 2026-11 |
| 19 | Grok Bot Realistic Guide (Creators' AI) | https://thecreatorsai.com/p/i-gave-grok-bot-a-real-job-realistic | 2026-09-17 |
| 20 | Grok Bot Computer Use: Complete Guide (AI Profit Boardroom) | https://aiprofitboardroom.com/blog/grok-bot-computer-use/ | 2026-10-31 |
| 21 | Grok is a bot, Claude is a desktop, Google is a lobby (ThinkReview) | https://thinkreview.dev/blog/grok-is-a-bot-claude-is-a-desktop-google-is-a-lobby | Page shows 2026-11-25 (later than today; use with care) |
| 22 | chiefbot issue #53: The bot's computer: a desktop with a dock, a waking-up screen and full screen | https://github.com/JorgeMenaDev/chiefbot/issues/53 | 2026-10-20 |
| 23 | OpenMausBot PR #3218: Take control right where you watch the bot's computer | https://github.com/milind-soni/OpenMausBot/pull/3218 | Not known |
| 24 | OpenMausBot PR #4824: Full screen watches the bot's screen without taking control | https://github.com/milind-soni/OpenMausBot/pull/4824 | Not known |
| 25 | Grok Bot glossary (grokbotwiki.com) | https://www.grokbotwiki.com/guides/grok-bot-glossary | Not known |
| 26 | xAI Launches Grok Bot (Unite.AI) | https://www.unite.ai/xai-launches-grok-bot-always-on-ai-teammates-with-their-own-cloud-computers/ | 2026-08 |
| 27 | A Field Guide to Grok Bot (X post by @undefinedKi) | https://x.com/undefinedKi/status/2113972662506541522 | Not known (we saw only a search snippet) |
| 28 | YouTube short: The Hidden Computer Your Grok Bot Agents Work On | https://www.youtube.com/shorts/brYzeaf8jT4 | Not known (not read) |

Notes on sources:
- We found no Reddit threads with UI details.
- Sources 22 to 24 are open-source clone projects. They describe Grok Bot from a static read of its app code (chiefbot cites Grok Bot 0.72.0). Use them as "reported" only.

### Timeline (confirmed unless noted)

- 2026-08-11: Beta launch on macOS and iOS (source 1).
- 2026-08-19: Windows desktop build (reported, community changelog).
- 2026-09-03: Design post (source 2). Enterprise access opens.
- 2026-09 to 2026-10: versions 0.49 to 0.78 (source 18).
- 2026-11-11: Grok Bot 1.0. Team Bots in Slack and Microsoft Teams. Bots in video calls (source 3).

---

## 2. Screens

### Overall frame (confirmed)

The app is a messaging app. It is not a dashboard.

- **Left:** a sidebar that lists Bots, not past chats (sources 2, 14, 17).
- **Center:** one conversation per Bot. A Bot has "one thread by design" (source 17).
- **Right:** a details pane. It opens when you click the Bot name in the chat header (sources 6, 18 v0.58.0). It has a profile at the top, then tabs "Media", "Computer", and "Routines" (source 18 v0.75.2). A "Library" tab was added in v0.62.0. When the pane opens or closes, the chat slides with it (v0.77.0).
- **Title bar:** a computer icon. It "turns purple while the computer is active" (source 2).
- **Full-screen computer view:** it fills the app window. It covers the chat (source 2).

The design post names five core objects: Bots, Chats, Prompts, Tools, Artifacts (source 2). The guiding rule: "As agents take on more responsibility, the interface should ask less of the person."

### (a) Bot roster / home

**Confirmed:**
- The roster is a **vertical list in the left sidebar**. It is not a grid. "The roster is the home screen" (source 21).
- When the user starts the app, the user lands on the synced Bot list (source 13). Setup creates the first Bot for the user (v0.51.0).
- Each row shows:
  - the Bot avatar (a simple shape with expressive eyes; see (b)),
  - the Bot name (a group chat shows its chat name, for example "Website launch"),
  - a one-line preview of the latest message or status, for example "Inbox's at 3. Two need a reply today." (source 2),
  - an unread dot (v0.63.0).
- Rows **do not show the time of the last message** since v0.52.0. (The September design mockup showed time stamps. The later app removed them.)
- There is **no live screen thumbnail** in the roster row.
- The primary Bot is "marked with a star" (v0.66.0).
- Pinned Bots stay at the top. The user can drag pinned Bots to change the order. A long pinned list folds with "Show less".
- Sections: the user can make, rename, reorder, and fold sections. The user drags Bots between sections. Right-click gives "Move to new section" and "Move to". There is an "Unassigned" section (v0.68.0). Sections sync between desktop and iOS.
- "Hidden Bots" is a folding section at the bottom of the sidebar (v0.57.0). Its action is "Show in sidebar". The row action is "Hide from sidebar".
- View menu: "View Unread" shows only Bots with unread messages (v0.70.2).
- Limit: about 50 Bots and group chats per account. A group chat has 2 to 6 Bots.
- The macOS dock badge counts unread conversations (v0.77.0). Muted Bots do not add to it.

**Sidebar top and bottom (confirmed):**
- Top: round buttons for "Search" and "New chat" (v0.58.0). The "+" button opens "New Bot" / "New Group Chat" (team accounts also see "Browse Team Bots").
- Bottom: the user name and an account menu. "Connect apps" is an icon beside the name (v0.77.0). Earlier it was a "Marketplace" button.

### (b) Bot card and status states

**Confirmed avatar system (source 2):**
- "simple shapes and expressive eyes". Variety comes from "controlled variations and accessories".
- The user picks a shape and a color when the user creates a Bot. A "pumpkin" shape exists (v0.75.5).
- The avatar must be easy to know "almost peripherally", without reading the name.
- Avatar edit tabs: "Bot" (shape), "Generate" (describe a picture), "Upload" (image under 25 MB). Button: "Set avatar".

**Confirmed status states (source 2):** idle, thinking, working, waiting, blocked, done.
- The app shows state **through avatar motion**, not through a separate badge. At rest the avatar is "calm and slightly curious". It "kicks into gear" when it works. It changes when it waits or needs help. It settles when the task is done.
- xAI tested three animated dots and rejected them. The dots did not show the difference between "working" and "stuck".
- The user can hover to see the current action.

**Confirmed sidebar attention states (source 10, 18):**
- "Needs attention"
- "Unread activity" (unread dot)
- Working or typing status
- "Waiting for you" (clears when the Bot stops waiting, v0.57.0)
- After a permission request, the row shows "Allowed", "Denied", or "Expired" (v0.71.2). Before that it showed "Permission required".
- If the list does not load in the folded sidebar, the app shows a warning icon with retry.

**Working phrases (confirmed examples):** "Thinking", "Working on it", "Thinking it through", "Working on your computer", "Digging around", "is on it", "Waiting on results". Specific phrases show at once. Generic phrases ("Working on it", "Thinking") wait a short time first (v0.78.1). The status line shows right after the user sends (v0.75.8).

**Not confirmed:** a colored status dot on the avatar. Grok Bot uses motion and text instead. For our app, see section 3.

### (c) Per-bot chat

**Confirmed layout:**
- Header: the Bot avatar and name. A click on the name opens the details pane ("View conversation details"). The computer icon is in the title bar.
- Transcript: a "heterogeneous transcript" (source 2). It mixes:
  - normal messages from the user and the Bot,
  - system events, for example "6 messages with Kenny, Tyler and Jenny" (a summary of Bot-to-Bot talk),
  - tool activity and computer use,
  - created files, charts, and Pages (the user can open them in the app),
  - questions, approval cards, and draft cards,
  - routine run results.
- An idle 1:1 chat with no messages shows only the animated avatar (v0.70.2).
- Composer at the bottom. Placeholder pattern: "Message <Bot name>" (source 2 mockup shows "Message Kenny"). The composer accepts text, links, images, files, `@` mentions (Bot, group, routine, app), and skill names. A waveform button "Start voice chat" shows only when the composer is empty. Dictation: "Start voice input".
- The user can send more messages while work runs. A user message outranks background work. The message "Stop now" stops work.
- In-app errors show **above the composer** under "Notifications". The user can close them one at a time or all at once. Some show "Copy request ID".
- Message actions: "Reply", threads, reactions, "More message actions", "Copy request ID".

**Where the live computer view sits (confirmed, source 2):** xAI tested and rejected four layouts:
- floating window (it covered the chat),
- side by side (it made users watch the Bot),
- modal (it treated the Bot workspace as an interruption),
- full screen as default (it pushed the chat away).

The final design has three levels:
1. **Status:** the title-bar icon turns purple when the computer is active.
2. **Preview:** "a pinned side panel" in the conversation. This is the right-hand details pane. The computer preview is at the top of the pane (sources 8, 27). The chat stays on the left (source 19).
3. **Takeover:** the user opens the computer full screen, takes control, and then hands it back.

**Streaming:** not confirmed. No source describes token-by-token streaming. The status line and the activity line show progress while the Bot works.

### (d) Live "computer" view

**Confirmed:**
- The user opens it with a click on the computer preview in the right-hand panel (source 8). The "Computer" tab in the details pane holds it.
- The user sees clicks, typing, page changes, and the current status.
- The computer is a cloud Linux desktop with a browser, files, and a terminal. Screen size is 1920 x 1200 (v0.68.1; before, 1280 x 800).
- It has a wallpaper. Dynamic wallpapers get lighter in the morning and darker at night. This gives the Bot's machine its own sense of time (source 2).
- The view follows the browser tab that the Bot brings to the front (v0.78.1).
- "Take over shows the screen your Bot is actually using" (v0.51.0). Each Bot has its own screen.
- Full screen opens the live view at once, with no second "Connecting" screen (v0.68.0).
- Full screen opens in **"Watching"** mode until the user presses **"Take control"** (v0.70.2).
- Computer states and labels:
  - "Asleep" / "Computer is asleep" / "Your computer is asleep". A click on the asleep preview wakes it in place (v0.71.2). An older version had a "Wake" button.
  - "Waking up…" (the Bot avatar with moving eyes and a shimmer title; reported by source 22).
  - "Reconnecting", "Unreachable", "Couldn't Reach Grok Bot's Computer", with "Retry".
  - "Setting up your Grok Bot…", "Updating Grok Bot's Computer", "Starting your computer…".
- The user can leave the preview. Work goes on. Closing the app or the laptop does not stop cloud work.

**Reported (source 22, clone of 0.72.0):**
- A small dock at the bottom center of the Bot desktop (browser, files, terminal).
- A floating pill below the screen: "Watching | Take control" while the Bot drives; "You're in control | Stop" while the person drives.
- An expand button makes the screen fill the app window. A collapse button or Esc exits.

**Confirmed on the product page mockup (source 4):** the labels "Computer", "You're in control", and "Working". A cursor labelled "You" shows when the human drives.

### (e) Activity / tool timeline

**Confirmed:**
- Grok Bot keeps progress quiet on purpose. The main signal is avatar motion plus a short working phrase. The user hovers to see the current action (source 2).
- xAI tested a one-line action description. Then users wanted every step. Research showed that users mainly wanted to feel sure the Bot was working (source 2).
- The design post shows a step log with entries like "Edited math.ts +14 −10", "Ran full suite 212 passed", and "Committed fix: clamp NaN". This is a compact list: a verb, an object, and a small result.
- The transcript shows tool activity, computer use, and created files as their own items (source 7).
- Reviewers say the view is too simple. One said the app "keeps the interface quiet while a bot works" and did not show files and commands like Claude Code does (source 16).
- Routine runs show in the transcript. The details pane "Routines" tab lists routines (for example "Morning briefing", "Weekly team update" marked "Paused"). Right-click gives "Pause, Resume, Test, Edit, and Delete".

**Approval cards (confirmed, source 11):**
- Standard: "Allow once", "Always allow", "Deny". After "Always allow": "Saved to Custom Rules".
- Terminal command: "Allow", "Skip", "Manage settings".
- Paused by an "Ask first" rule: "Allow", "Not now", "Edit rule".
- Warnings on cards: "This can't be undone", "This changes sign-in or security settings", "This shares your data outside this chat".
- Background cards expire after 30 minutes and show "Expired".
- Draft cards: "Needs approval", with "Send" and "Discard".
- Suggestion card: "Suggestion from <Bot>" with "Update" and "Not now" (source 4).

### (f) Login hand-off / take-over and hand-back

**Confirmed flow (sources 8, 9, 12, 13):**
1. The Bot reaches a step that it cannot do. Examples: SSO, passkey, CAPTCHA, QR code, approval on another device, bank card check.
2. The Bot asks in chat. Product page example: "Sign in to Zendesk so I can work the support queue."
3. The **"Computer"** card in the chat shows **"Action needed"**. The card has a **"Take over"** button.
4. Or: the user opens the computer and presses **"Take control"**.
5. The full-screen view changes from "Watching" to "You're in control" (reported pill label). The user does only the blocked step.
6. The user presses **"I'm done"**. Then the user tells the Bot to continue. (On mobile, "I'm done" also returns control.)
7. The session stays signed in on the computer. All Bots on the account share it.

**Related confirmed details:**
- A browser sign-in waits up to 20 minutes for the user (v0.75.2).
- When the sign-in is for Google, GitHub, or QuickBooks, the hand-off offers the matching connector. Connecting ends the hand-off (v0.77.0).
- A security-key sign-in during take-over opens the security key prompt (v0.77.0).
- Fingerprint, Face ID, Windows Hello, and local passkeys do not work on the Bot computer.

**Secure Form (no take-over needed):** for passwords and cards, the Bot uses a Secure Form first. Text: "Never shown to your Bot. Filled directly to example.com." Checkbox: "Securely save to Bot's vault". Success: "Filled into the page. Secret values were never shown to your Bot." Failure: "Could not fill into the page".

**Banner text during take-over:** not confirmed beyond the pill labels. See section 3 for our design.

### (g) Create a bot

**Confirmed (source 6):**
1. Press "New Bot" (+) at the top of the sidebar, or Cmd/Ctrl+N.
2. Dialog "Create New Bot": enter a name, choose a shape and a color, press "Create".
3. The chat with the new Bot opens.
4. The first message sets the Bot's job, sources, and approval limits. The Bot starts an onboarding talk. It asks what the user needs and which tools it must connect (source 14).
- First run: setup makes the first Bot ("primary Bot") for the user. Prompt: "Introducing your primary Bot", with "Create primary Bot" or "Choose an existing Bot" → "Confirm". Guides suggest a "Chief of Staff" template first (source 17).
- Templates: "Connect apps" → "Bot templates" → "Import Bot". Product page job examples: "Sales Outbound", "Talent Scout", "Paid Media", "Expense Manager", "Product Performance", "Bug Reproduction", "Account Health", "Chief of Staff".
- Other Bots can make a Bot. It shows in the sidebar (v0.53.0).
- Personal Bots have no description field and no settings page. The user changes behavior in chat.
- Teach a task: the Bot watches the user do a job once, then saves it as a routine. Label: "<Bot> is watching and learning", "Teach a task", with a timer (source 4).

### (h) Settings

**Confirmed (source 10):**
- Open with the account menu → "Settings", or Cmd/Ctrl+, . Dialog title "Grok Bot settings".
- Panes: **General**, **Computer**, **Vault**, **Usage & Billing**, **Updates**.
- General: Account (sign in/out, "Switch account", "Add account"); Appearance (theme "Follow System" / "Light" / "Dark", "Accent": "Bot color" or "Black", "Language"); System ("Microphone", "Use hardware acceleration"); Bot ("Timezone", "Auto-review" switch, "Custom Rules"); Import ("Import Memory to Grok Bot"). There is no model picker.
- Computer: "Your computer" ("Execution on this computer": "Ask every time" / "Always allow" / "Never allow"; "Route traffic through this computer") and "Bot's computer" ("Update Bot's Computer", "Recover computer", "Reset Bot's Computer"). Settings name the Bots that an update would stop (v0.78.1).
- Vault: "New item" → "Login" / "Card" / "Contact info". "Import from CSV…".
- Usage & Billing: "My plan", "Usage limits", "Grok Bot usage" ("By Bot" / "By source"), "Additional usage", "Resets".
- Updates: "Check for Updates", "Restart to Update", "Automatic Updates".
- Per-Bot settings: in the details pane, through the Bot name in the header. Shortcut Cmd/Ctrl+Shift+, ("Toggle Bot settings"). The user edits name, label ("Add a label"), and avatar.

### (i) Empty, error, and offline states

**Confirmed labels (sources 12, 13, 18):**
- Offline app: "You're offline" with "Grok Bot refreshes on its own when you're back online". The open chat may show "Showing saved messages".
- Computer: "Reconnecting", "Unreachable", "Couldn't Reach Grok Bot's Computer" → "Retry". Further steps: "Update Bot's Computer", "Recover computer" (dialog "Recover Grok Bot's Computer"), "Reset". Other buttons: "Continue in Background", "Keep waiting", "Retry Recovery", "Retry Reset".
- Slow update: "Update still running", "Taking longer than usual".
- Asleep computer: "Your computer is asleep". The app keeps what it loaded.
- Bot errors: "Bot failed to respond", "Model provider is overloaded".
- Hung Bot: it "stops itself with a note" (v0.66.0).
- Usage: "Your plan doesn't include Grok Bot usage", "This Bot's owner has reached their usage limit", banner with "Use Reset".
- Team: "Bot Not Found", "No Access to This Bot".
- Loading errors inside panes: "Couldn't load routines" (inside the section), "Try Again".
- Empty: idle 1:1 chat shows only the animated avatar. If all Bots are hidden, home shows "Show Hidden Bots". Secrets: "No secrets yet".
- Forced update after 14 days: "Update required" → "Restart to update".
- The app does not show service status. The user checks status.cursor.com.

### (j) Sidebar / navigation and keyboard shortcuts

**Confirmed shortcuts (source 7; Cmd on macOS, Ctrl on Windows/Linux):**

| Action | Keys |
|---|---|
| Jump to / search palette | Cmd/Ctrl+K |
| Search Bots | Cmd/Ctrl+Shift+F |
| New Bot | Cmd/Ctrl+N |
| New group chat | Cmd/Ctrl+Shift+N |
| Show all shortcuts | Cmd/Ctrl+/ |
| Find in this chat | Cmd/Ctrl+F |
| Compact sidebar | Cmd/Ctrl+B |
| Focus prompt | Cmd/Ctrl+I |
| Add selected text to chat | Cmd/Ctrl+L |
| Sidebar Bot 1 to 9 | Cmd/Ctrl+1 to 9 |
| Previous / next Bot | Alt+Up / Alt+Down |
| Back / forward | Cmd/Ctrl+[ / Cmd/Ctrl+] |
| Cycle Bots | Control+Tab / Control+Shift+Tab |
| Connect apps | Cmd/Ctrl+Shift+M |
| Toggle Bot settings | Cmd/Ctrl+Shift+, |
| Conversation details | Cmd+Shift+I or Cmd+Alt+B (macOS); Ctrl+Alt+B (Windows/Linux) |
| Send | Enter (or Cmd/Ctrl+Enter from anywhere) |
| New line | Shift+Enter |
| Dictate | Cmd/Ctrl+D (hold to talk) |
| Settings | Cmd/Ctrl+, |
| Zoom in / out / reset | Cmd/Ctrl+= / Cmd/Ctrl+- / Cmd/Ctrl+0 |
| Full screen | F11 (macOS also Control+Cmd+F) |

- The command palette (Cmd/Ctrl+K) shows Bots, settings, actions, and routines before messages (v0.54.0). On a Mac, Ctrl-N and Ctrl-P move in it.
- Each Bot reopens on the tab that the user had open last (v0.51.0).
- Clicking the current Bot in the sidebar closes an open Page (v0.78.1).
- Esc to leave full-screen computer view: reported (source 22), not confirmed.

---

## 3. Screens we will match

Target: a desktop window of about 1280 x 800. Use a three-column layout.

Base frame:
- Title bar: 36 px. App name at left. A computer-status icon at right. The icon is grey when no computer is active and accent color (purple) when a Bot computer is active.
- Left sidebar: 260 px (64 px when compact, Cmd/Ctrl+B).
- Center chat: flexible width (min 480 px).
- Right details pane: 360 px. It is closed by default. When it opens, the chat gets narrower (no overlay).
- Light and dark themes. One accent color per Bot (the Bot color).

### 3.1 Roster (sidebar and home)

- Top row: "Search" (Cmd/Ctrl+K) and "+" (menu: "New Bot", "New group chat").
- Sections: "Pinned", then user sections, then "Unassigned". Each section can fold. The user drags rows between sections.
- Row (height 52 px): avatar 32 px (shape + eyes + Bot color), name (bold, one line), one-line preview below (grey, one line, end with "…").
- Status on the row:
  - Avatar motion for idle / working / waiting / done (respect "reduce motion": then use a 8 px dot at the avatar's lower right: grey idle, accent working, amber waiting, red blocked/error, none when done).
  - The preview line shows the working phrase while the Bot works (for example "Working on your computer").
  - Badge "Needs attention" (amber pill) when the Bot waits for the user (approval, sign-in, question).
  - Unread dot (8 px, accent) at the right edge.
  - No time stamp. No live screen thumbnail.
- Hover on a working row: a tooltip with the current action (for example "Opening acme.zendesk.com").
- Bottom: "Hidden Bots" (folding section), then the user name, a settings icon, and a "Connect apps" icon.
- Home (no Bot selected): show the primary Bot chat. If there are no Bots: center empty state with a large idle avatar, the text "Create your first Bot", and a "New Bot" button.
- Keyboard: Cmd/Ctrl+1 to 9, Alt+Up/Down, Control+Tab.

### 3.2 Bot chat with streaming and tool activity

- Header (56 px): avatar, name (click opens details pane), label under the name, and at right: the computer icon button (opens details pane on the "Computer" tab) and a "…" menu.
- Transcript (max text width 720 px, centered in the column):
  - User messages: right side, filled bubble.
  - Bot messages: left side, no bubble, avatar at the first message of a group.
  - **Streaming:** Bot text shows token by token with a blinking caret at the end. Markdown renders while it streams. A "Stop" button replaces "Send" while the Bot streams or works.
  - **Status line** (above the composer, or as the last transcript item): animated avatar 16 px + a working phrase ("Thinking", "Working on your computer", "Waiting on results"). Show specific phrases at once. Show generic phrases after 600 ms.
  - **Activity group:** tool calls from one turn fold into one row: "Used 4 tools" with a chevron. When open, show a compact list: icon, verb + object, small result at right (for example "Ran tests — 212 passed"). The running item has a spinner. A failed item is red with "Retry".
  - **Computer card:** when the Bot uses its computer, show a small card in the transcript: "Computer", a short state text ("Working"), and an "Open" button.
  - **Approval card:** title, proposed action, inputs, warning line if needed, buttons "Allow once", "Always allow", "Deny". After an answer: "Allowed", "Denied", or "Expired".
  - **Draft card:** "Needs approval" tag, editable body, "Send" and "Discard".
- Composer (bottom, min 56 px, grows to 40% of height): placeholder "Message <Bot name>", attach button, `@` mentions, Send (Enter), new line (Shift+Enter).
- Notice area above the composer for errors, with close and "Copy request ID".

### 3.3 Bot screen with take-over

- **Level 1, status:** title-bar computer icon in accent color while the Bot computer is active.
- **Level 2, preview:** details pane, tab "Computer". At the top: a live 16:10 preview (328 x 205 px). Below it: state text ("Working", "Asleep", "Waking up…") and the Bot's current action. A click on the preview opens full screen. A click on an asleep preview wakes the computer in place.
- **Level 3, full screen:** the view fills the window content area (chat hidden). Keep a 44 px top bar: Bot avatar + name, state, and at right "Exit full screen" (Esc).
  - Default mode **"Watching"**: the user sees the live screen. Input does not go to the Bot computer. A floating pill at bottom center: "Watching" | button "Take control".
  - **Take-over mode:** a 2 px accent border around the screen. Top banner (accent background): "You're in control. <Bot name> is paused." Pill: "You're in control" | button "I'm done". Keyboard and mouse go to the Bot computer. Show a "You" cursor label.
  - **Hand-back:** "I'm done" returns control. The pill goes back to "Watching". The app posts an event in the chat ("You signed in. <Bot name> continues.") and sends a continue message to the Bot.
- **Sign-in request flow:** the Bot posts a Computer card with the tag "Action needed", the text of its request (for example "Sign in to example.com so I can …"), and a "Take over" button. The sidebar row shows "Needs attention". "Take over" opens full screen directly in take-over mode. A countdown shows "Waiting for you · 20 min".
- Computer states in the screen area: "Waking up…" (animated avatar, shimmer), "Your computer is asleep" (click to wake), "Reconnecting…", "Couldn't reach <Bot name>'s computer" + "Retry".

### 3.4 Sign-in (app account)

- Window centered card (400 px wide) on a plain background.
- App name, one line of text: "Your team of always-on Bots."
- Primary button "Sign in". Under it, small text link "Use a different account".
- During the browser hand-off: spinner and "Finish signing in in your browser." with "Cancel" and "Open browser again".
- Error: red line under the button ("Sign-in failed. Try again.") and "Try again".
- After first sign-in: "Setting up your first Bot…" then open the primary Bot chat.

### 3.5 Settings

- Modal dialog 720 x 560 px. Left list of panes (180 px). Right content. Title "Settings". Open with Cmd/Ctrl+, . Close with Esc.
- Panes:
  - **General:** account (signed-in name, "Sign out"), theme ("Follow System" / "Light" / "Dark"), accent ("Bot color" / "Black"), notifications switch, timezone.
  - **Computer:** state of the Bot computer, "Update computer", "Recover computer", "Reset computer" (red, with a confirm dialog that lists the Bots it will stop).
  - **Approvals:** "Auto-review" switch, "Custom Rules" list with "Add Rule".
  - **Vault:** list of saved logins, "New item".
  - **Updates:** version, "Check for updates", "Automatic updates" switch.
- Per-Bot settings: in the details pane profile area (name, label, avatar), not in this dialog.

### 3.6 Error and offline

- **App offline:** a 32 px banner under the title bar, grey: "You're offline. The app refreshes on its own when you're back online." The open chat shows "Showing saved messages" at the top. The composer is disabled with the placeholder "You're offline".
- **Bot computer unreachable:** in the Computer tab and full screen: the Bot avatar (still), the text "Couldn't reach <Bot name>'s computer", a primary "Retry" button, and a secondary "Recover computer" link.
- **Reconnecting:** small pill over the screen "Reconnecting…" with spinner. No full block.
- **Bot failed:** in the transcript, a red-edge notice "Bot failed to respond" with "Retry" and "Copy request ID".
- **Provider overloaded:** notice above the composer: "Model provider is overloaded. Try again soon." with "Retry".
- **Sidebar load error:** a warning icon at the top of the list with "Retry".
- **Empty states:** no Bots → "Create your first Bot"; Bot with no messages → only the large animated avatar and the composer; no routines → "No routines yet"; all Bots hidden → "Show Hidden Bots".
