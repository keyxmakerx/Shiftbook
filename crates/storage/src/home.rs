//! "What's Shiftbook for?" (round 15) and the last day Shiftbook was open. Picking a mode
//! saves it, puts its defaults in place (daily targets, auto-lock) and shows or hides the
//! starter categories and kinds of repeating things (see `shiftbook_core::home`). Only
//! starter values are touched, never ones the user added, and hiding never changes history.

use shiftbook_core::dates::{format_date, parse_date};
use shiftbook_core::home::{
    category_visibility, mode_defaults, recurring_type_visibility, UseMode,
};
use shiftbook_core::ids::OwnerId;
use shiftbook_core::model::ListName;
use shiftbook_core::prefs::{KEY_AUTO_LOCK, KEY_DAY_TARGETS, KEY_LAST_OPEN, KEY_USE_MODE};
use time::Date;

use crate::repo::{now_text, ListOptionRepository, Result, SettingsRepository};
use crate::SqliteStore;

impl SqliteStore {
    /// Pick what Shiftbook is for and put that mode's defaults in place.
    pub async fn set_use_mode(&self, owner: OwnerId, mode: UseMode) -> Result<()> {
        let defaults = mode_defaults(mode);
        self.set_pref(owner, KEY_USE_MODE, mode.as_str()).await?;
        let targets = defaults.day_targets.map(|n| n.to_string()).join(",");
        self.set_pref(owner, KEY_DAY_TARGETS, &targets).await?;
        self.set_pref(
            owner,
            KEY_AUTO_LOCK,
            &defaults.auto_lock_minutes.to_string(),
        )
        .await?;
        self.show_starters(owner, ListName::Category, &category_visibility(mode))
            .await?;
        self.show_starters(
            owner,
            ListName::RecurringType,
            &recurring_type_visibility(mode),
        )
        .await
    }

    async fn show_starters(
        &self,
        owner: OwnerId,
        list: ListName,
        wanted: &[(&str, bool)],
    ) -> Result<()> {
        let existing = self.list_options(owner, list, true).await?;
        for (value, shown) in wanted {
            match existing
                .iter()
                .find(|o| o.value.eq_ignore_ascii_case(value))
            {
                Some(o) if o.hidden == *shown => {
                    sqlx::query(
                        "UPDATE list_options SET hidden = $1, updated_at = $2 \
                         WHERE id = $3 AND owner_id = $4",
                    )
                    .bind(!*shown)
                    .bind(now_text())
                    .bind(o.id.to_string())
                    .bind(owner.to_string())
                    .execute(&self.pool)
                    .await?;
                }
                Some(_) => {}
                None if *shown => {
                    self.add_list_option(owner, list, value, None).await?;
                }
                None => {}
            }
        }
        Ok(())
    }

    /// The last day Shiftbook was open, if known.
    pub async fn last_open(&self, owner: OwnerId) -> Result<Option<Date>> {
        Ok(self
            .get_setting(owner, KEY_LAST_OPEN)
            .await?
            .and_then(|v| parse_date(&v).ok()))
    }

    /// Remember that Shiftbook was open on `today`.
    pub async fn set_last_open(&self, owner: OwnerId, today: Date) -> Result<()> {
        self.set_pref(owner, KEY_LAST_OPEN, &format_date(today))
            .await
    }
}
