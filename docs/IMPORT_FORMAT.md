# Import / Export format (schema_version 1)

The same JSON format is used for **Export JSON**, **Import JSON** and the one-time import from the
Excel tracker (`tracker-import.json`, kept OUTSIDE the repo).

```jsonc
{
  "format": "work-tracker-export",
  "schema_version": 1,
  "exported_at": "2026-10-15T15:00:00+00:00",
  "source": "Engineering Time Tracker.xlsx",
  "owner": { "id": "uuid", "display_name": "..." },
  "lists": {                       // ordered values for dropdowns
    "category": ["..."], "status": ["..."], "priority": ["..."], "recurring_type": ["..."]
  },
  "frequencies":     [{ "id", "label", "months", "days" }],
  "timesheet_codes": [{ "id", "area", "phase_label", "code", "sort_order" }],
  "category_code_defaults": [{ "category", "timesheet_code_id" }],   // suggestions; review on import
  "projects":        [{ "id", "owner_id", "name", "description", "requested_by", "due_date",
                        "status", "priority", "estimated_minutes", "created_at", "updated_at" }],
  "recurring_tasks": [{ "id", "owner_id", "name", "type", "frequency_id", "expiration_date",
                        "warn_days_before", "last_done_before_tracker", "notes",
                        "created_at", "updated_at" }],
  "time_entries":    [{ "id", "owner_id", "date", "minutes",
                        "target": { "type": "project|recurring|none", "id": "uuid|null" },
                        "category", "description", "status", "priority", "next_step",
                        "tickets": [], "legacy_ticket_text", "created_at", "updated_at" }],
  "tickets": [],
  "tasks": []
}
```

## Import rules
- Validate `format` and `schema_version`; reject unknown versions with a clear message.
- Upsert by `id` (UUID) — importing the same file twice must not create duplicates.
- Missing list values referenced by records are added to the lists automatically.
- `owner_id` is remapped to the current user on import.
- Recurring tasks named like routine work (`General - Tickets`, `Meeting - ...`) or with frequency
  `Daily` should be offered as `alerts_enabled = false` in an import preview step.
- `legacy_ticket_text` (free text from Excel, e.g. "Multiple") is shown on the entry as a note; it
  is not split into tickets automatically.
- Show a preview (counts per entity, conflicts) before committing; run in one transaction.
