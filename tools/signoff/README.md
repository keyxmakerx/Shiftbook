# Sign-off sheet

A live page where Claude brings decisions, mockups and screenshots, and the owner approves or asks
for changes. Live copy: https://claude.ai/artifact/UxX7VX1HE8E53TR5VkEdiC (private to the owner; from round 15.
Earlier rounds are on https://claude.ai/artifact/AZ8PuPVzRcToWrxj33Ku8T and, before that,
https://claude.ai/artifact/FDWT4t9XNKtL2n22wvJcx1).

The page (`index.html`) is a fixed shell. All content lives in the artifact's database, so new
rounds arrive live and the page never needs a reload or republish to show them.

## Collections
| Path | Written by | What |
|---|---|---|
| `cards/<id>` | Claude (owner-only rule) | One decision card. See `rounds/r01.json` for the shape. |
| `meta/board` | Claude (owner-only rule) | Current round number, heading, intro. |
| `decisions/<card id>` | The page | `{choice: approve\|change\|draft, note, answers, version, title, at, edited}` |
| `notes/<id>` | The page, then Claude | `{text, at}` from the owner; Claude adds `seen`, `reply`, `replied_at`. |

## Adding a round
1. Write `rounds/rNN.json` (cards + meta). Mockups go in `mocks/` and are referenced as
   `"figures": [{"html_file": "mocks/x.html", "caption": "..."}]`; images as `{"src": "<asset url>"}`.
2. `python3 tools/signoff/build_round.py tools/signoff/rounds/rNN.json OUT_DIR`
3. Write each output file with an ArtifactData `batch` (`set` on `cards/<id>`, `meta/board`).
4. Archive the previous round: `update` its cards with `{"archived": true}`.

Changing a card: bump its `version`. A decision made on an older version shows as **Re-check**.
If the owner is typing in that card, the page offers "Show the new version" instead of swapping it.

## Rules the page keeps
- Never replaces a card that has focus or unsaved typing; drafts are kept in localStorage until
  the store confirms them.
- Mockup HTML runs in a sandboxed iframe (`allow-scripts` only), and all other card text is set
  as text, never as HTML.
- Approved decisions are copied into `docs/DECISIONS.md`.
