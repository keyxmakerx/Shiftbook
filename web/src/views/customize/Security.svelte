<script lang="ts">
  // Settings → Security: change password, auto-lock time, Quick unlock with a PIN (round 15),
  // a new recovery key and deleting all data.
  import { api } from '../../lib/api';
  import type { FingerprintStatus } from '../../lib/api';
  import { CLEAR_AFTER_MS, copySecret } from '../../lib/clipboard';
  import { errorMessage } from '../../lib/messages';
  import { recoveryGroups } from '../../lib/recoveryKey';
  import { toIsoDate } from '../../lib/dates';
  import { app, attempt, savePref } from '../../lib/store.svelte';
  import { toast } from '../../lib/toast.svelte';
  import ChangePassword from '../../components/ChangePassword.svelte';
  import DeleteAllData from '../../components/DeleteAllData.svelte';
  import { phone } from '../../lib/phone.svelte';
  import { planNudges } from '../../lib/launch.svelte';

  const LEAVE_CHOICES = [
    { minutes: 0, label: 'Straight away' },
    { minutes: 1, label: 'After 1 minute' },
    { minutes: 5, label: 'After 5 minutes' },
    { minutes: 15, label: 'After 15 minutes' },
  ];
  import PinBoxes from '../../components/PinBoxes.svelte';

  let changing = $state(false);
  let minutes = $state(app.prefs?.autoLockMinutes ?? 15);
  let password = $state('');
  let working = $state(false);
  let keyError = $state('');
  // Held only until "I saved it" is pressed.
  let newKey = $state<string | null>(null);
  let copied = $state(false);
  let deleting = $state(false);

  // Quick unlock: turning it on asks for the password and a PIN typed twice.
  let pinOn = $state(false);
  let pinPassword = $state('');
  let pin1 = $state('');
  let pin2 = $state('');
  let pinError = $state('');
  let pinWorking = $state(false);
  let readyToday = $state(false);
  // Just set today's PIN (no password: the app was unlocked with it today).
  let settingToday = $state(false);
  let todayPin = $state('');

  $effect(() => {
    void app.prefs?.quickUnlock;
    void api.quickUnlockReady(toIsoDate(new Date())).then((r) => (readyToday = r), () => (readyToday = false));
  });

  async function turnOn(event: SubmitEvent) {
    event.preventDefault();
    if (pinWorking || !pinPassword) return;
    if (pin1.length !== 6 || pin1 !== pin2) {
      pinError = pin1.length !== 6 ? 'A PIN is 6 digits.' : "The two PINs don't match.";
      return;
    }
    const [password, pin] = [pinPassword, pin1];
    pinPassword = pin1 = pin2 = '';
    pinError = '';
    pinWorking = true;
    try {
      await api.enableQuickUnlock(password, pin, toIsoDate(new Date()));
      const p = await api.loadPrefs();
      app.prefs = p;
      pinOn = false;
      readyToday = true;
      toast('Quick unlock is on. Until midnight, locks open with your PIN.');
    } catch (e) {
      pinError = errorMessage(e);
    } finally {
      pinWorking = false;
    }
  }

  async function setToday(pin: string) {
    todayPin = '';
    const ok = await attempt(() => api.startQuickUnlock(pin, toIsoDate(new Date())).then(() => true));
    if (!ok) return;
    settingToday = false;
    readyToday = true;
    toast("Today's PIN is set.");
  }

  async function turnOff() {
    await attempt(() => api.endQuickUnlock());
    await savePref('security.quick_unlock', 'false');
    readyToday = false;
    toast('Quick unlock is off. Locks ask for your password.');
  }

  async function saveMinutes() {
    const p = await attempt(() => api.savePref('security.auto_lock_minutes', String(minutes)));
    if (p) {
      app.prefs = p;
      toast(`Shiftbook locks after ${p.autoLockMinutes} minutes without activity.`);
    }
  }

  let remindTime = $state(app.prefs?.remindAt ?? '08:00');

  // Phone: fingerprint unlock, off until turned on here with the password.
  let finger = $state<FingerprintStatus | null>(null);
  let fingerPassword = $state('');
  let fingerWorking = $state(false);
  let fingerError = $state('');
  $effect(() => {
    if (phone.on) void api.fingerprintStatus().then((s) => (finger = s)).catch(() => undefined);
  });

  async function fingerOn(event: SubmitEvent) {
    event.preventDefault();
    if (!fingerPassword || fingerWorking) return;
    const typed = fingerPassword;
    fingerPassword = '';
    fingerError = '';
    fingerWorking = true;
    try {
      await api.fingerprintOn(typed);
      toast('Fingerprint unlock is on. After the phone restarts, the password is asked for once.');
    } catch (e) {
      fingerError = errorMessage(e);
    } finally {
      fingerWorking = false;
      finger = await api.fingerprintStatus().catch(() => finger);
    }
  }

  async function fingerOff() {
    await attempt(() => api.fingerprintOff());
    finger = await api.fingerprintStatus().catch(() => finger);
    toast('Fingerprint unlock is off.');
  }

  /** Turning a nudge on asks Android for permission to notify (from Android 13). */
  async function setNudge(key: 'phone.remind_at' | 'phone.wrapup_nudge', value: string, on: boolean) {
    if (!(await savePref(key, value))) return;
    if (on && !(await api.phoneAllowNotifications().catch(() => false))) {
      toast('Allow notifications for Shiftbook when Android asks, or in Android’s settings.');
    }
    await planNudges();
  }

  async function saveLeave(value: number) {
    const p = await attempt(() => api.savePref('security.leave_lock_minutes', String(value)));
    if (p) app.prefs = p;
  }

  async function makeKey(event: SubmitEvent) {
    event.preventDefault();
    if (!password || working) return;
    const typed = password;
    password = '';
    keyError = '';
    working = true;
    try {
      newKey = await api.regenerateRecoveryKey(typed);
      copied = false;
    } catch (e) {
      keyError = errorMessage(e);
    } finally {
      working = false;
    }
  }
</script>

<article class="paper cz-card">
  <h3>Password</h3>
  {#if changing}
    <ChangePassword onClose={() => (changing = false)} />
  {:else}
    <div><button class="btn" type="button" onclick={() => (changing = true)}>Change password</button></div>
  {/if}
</article>

<article class="paper cz-card">
  <h3>Lock automatically</h3>
  <p class="sub">
    {phone.on
      ? 'After this many minutes without a tap, while Shiftbook is open. The lock button locks at once.'
      : 'After this many minutes without keyboard or mouse use. Ctrl+L locks at once.'}
  </p>
  <form
    class="cz-row"
    onsubmit={(e) => {
      e.preventDefault();
      void saveMinutes();
    }}
  >
    <input class="text-in mins" type="number" min="1" max="240" bind:value={minutes} aria-label="Minutes before locking" />
    <span class="cz-grow">minutes</span>
    <button class="btn primary" type="submit">Save</button>
  </form>
</article>

{#if phone.on && finger?.available}
  <article class="paper cz-card">
    <h3>Unlock with fingerprint</h3>
    {#if finger.on}
      <p class="sub">On. After the phone restarts, Shiftbook asks for your password once.</p>
      <div><button class="btn" type="button" onclick={() => void fingerOff()}>Turn off</button></div>
    {:else}
      <p class="sub">Off until you turn it on. Needs your password once.</p>
      <form class="cz-row" onsubmit={fingerOn}>
        <input
          class="text-in cz-grow"
          type="password"
          bind:value={fingerPassword}
          autocomplete="off"
          placeholder="Your password"
          aria-label="Your password"
          disabled={fingerWorking}
        />
        <button class="btn primary" type="submit" disabled={!fingerPassword || fingerWorking}>
          {fingerWorking ? 'Waiting for your fingerprint…' : 'Turn on'}
        </button>
      </form>
      {#if fingerError}<p class="error" role="alert">{fingerError}</p>{/if}
    {/if}
  </article>
{/if}

{#if phone.on}
  <article class="paper cz-card">
    <h3>Lock after leaving the app</h3>
    <p class="sub">When you switch to another app or turn the screen off.</p>
    <div class="leave" role="radiogroup" aria-label="Lock after leaving the app">
      {#each LEAVE_CHOICES as choice (choice.minutes)}
        <button
          class="btn"
          class:primary={app.prefs?.leaveLockMinutes === choice.minutes}
          type="button"
          role="radio"
          aria-checked={app.prefs?.leaveLockMinutes === choice.minutes}
          onclick={() => void saveLeave(choice.minutes)}>{choice.label}</button
        >
      {/each}
    </div>
  </article>

  <article class="paper cz-card">
    <h3>Reminders on this phone</h3>
    <p class="sub">
      Quiet notifications on work days (days with a target in Your day). They show a count or a
      nudge, never what's due, so they're safe on the lock screen. Off unless you turn them on.
    </p>
    <div class="cz-row">
      <label class="check-row">
        <input
          type="checkbox"
          checked={!!app.prefs?.remindAt}
          onchange={(e) => {
            const on = (e.currentTarget as HTMLInputElement).checked;
            void setNudge('phone.remind_at', on ? remindTime : '', on);
          }}
        />
        Due reminder at
      </label>
      <input
        class="text-in mono time"
        type="time"
        bind:value={remindTime}
        aria-label="Due reminder time"
        onchange={() => app.prefs?.remindAt && void setNudge('phone.remind_at', remindTime, false)}
      />
    </div>
    <label class="check-row">
      <input
        type="checkbox"
        checked={!!app.prefs?.wrapupNudge}
        disabled={!app.prefs?.wrapupAt}
        onchange={(e) => {
          const on = (e.currentTarget as HTMLInputElement).checked;
          void setNudge('phone.wrapup_nudge', on ? 'true' : 'false', on);
        }}
      />
      Wrap-up nudge at {app.prefs?.wrapupAt ?? 'the wrap-up time'}
    </label>
    {#if !app.prefs?.wrapupAt}<p class="note">Turn on the wrap-up in Your day first.</p>{/if}
    <p class="note">Shiftbook plans the next reminder each time you open it.</p>
  </article>
{/if}

<article class="paper cz-card">
  <h3>Quick unlock with a PIN</h3>
  <p class="sub">
    After a lock, open Shiftbook with a 6-digit PIN instead of your password. It works on the day
    you type it, for up to 10 hours after you unlock with your password, and stops after the
    computer sleeps, when Shiftbook closes, or after 3 wrong PINs. The PIN is never saved.
  </p>
  {#if app.prefs?.quickUnlock}
    <div class="cz-row">
      <span class="cz-grow note">{readyToday ? 'On. Locks open with your PIN until midnight.' : "On. Today's PIN isn't set, so locks ask for your password."}</span>
      {#if !readyToday && !settingToday}
        <button class="btn" type="button" onclick={() => (settingToday = true)}>Set today's PIN</button>
      {/if}
      <button class="btn" type="button" onclick={() => void turnOff()}>Turn off</button>
    </div>
    {#if settingToday}
      <div class="cz-row">
        <PinBoxes bind:value={todayPin} label="Today's PIN" autofocus onfull={(p) => void setToday(p)} />
        <button class="btn" type="button" onclick={() => (settingToday = false)}>Cancel</button>
      </div>
    {/if}
  {:else if pinOn}
    <form class="pin-on" onsubmit={turnOn}>
      <label class="field">
        <span>Your password</span>
        <input class="text-in" type="password" bind:value={pinPassword} autocomplete="off" disabled={pinWorking} />
      </label>
      <div class="field">
        <span>A 6-digit PIN</span>
        <PinBoxes bind:value={pin1} label="A 6-digit PIN" disabled={pinWorking} />
      </div>
      <div class="field">
        <span>The PIN again</span>
        <PinBoxes bind:value={pin2} label="The PIN again" disabled={pinWorking} />
      </div>
      {#if pinError}<p class="error" role="alert">{pinError}</p>{/if}
      <div class="cz-row">
        <button class="btn primary" type="submit" disabled={pinWorking || !pinPassword}>
          {pinWorking ? 'Turning it on…' : 'Turn on Quick unlock'}
        </button>
        <button class="btn" type="button" onclick={() => (pinOn = false)} disabled={pinWorking}>Cancel</button>
      </div>
    </form>
  {:else}
    <div><button class="btn" type="button" onclick={() => (pinOn = true)}>Turn on Quick unlock</button></div>
  {/if}
</article>

<article class="paper cz-card">
  <h3>Recovery key</h3>
  {#if newKey}
    <p class="sub">Your new recovery key. The old one no longer works. Write it down and keep it away from this computer.</p>
    <ol class="key" aria-label="New recovery key">
      {#each recoveryGroups(newKey) as group, i (i)}
        <li><span class="num" aria-hidden="true">{i + 1}</span><code>{group}</code></li>
      {/each}
    </ol>
    <div class="cz-row">
      <button
        class="btn"
        type="button"
        onclick={async () => {
          copied = await copySecret(newKey ?? '');
        }}>Copy key</button
      >
      <span class="cz-grow note">
        {copied
          ? `Copied. The clipboard is emptied in ${CLEAR_AFTER_MS / 1000} seconds.`
          : 'Careful: clipboard history or sync tools can keep a copy.'}
      </span>
      <button class="btn primary" type="button" onclick={() => (newKey = null)}>I saved it</button>
    </div>
  {:else}
    <p class="sub">Make a new recovery key if the old one is lost. Your password is needed.</p>
    <form class="cz-row" onsubmit={makeKey}>
      <input
        class="text-in cz-grow"
        type="password"
        bind:value={password}
        autocomplete="off"
        placeholder="Your password"
        aria-label="Your password"
        disabled={working}
      />
      <button class="btn primary" type="submit" disabled={!password || working}>
        {working ? 'Making a new key…' : 'Make a new recovery key'}
      </button>
    </form>
    {#if keyError}<p class="error" role="alert">{keyError}</p>{/if}
  {/if}
</article>

<article class="paper cz-card">
  {#if deleting}
    <DeleteAllData onCancel={() => (deleting = false)} />
  {:else}
    <h3>Delete all data</h3>
    <p class="sub">Remove everything Shiftbook keeps on this computer and start again. This can't be undone.</p>
    <div><button class="btn danger" type="button" onclick={() => (deleting = true)}>Delete all data…</button></div>
  {/if}
</article>

<style>
  .leave {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }

  .time {
    width: 120px;
  }

  .pin-on {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .pin-on .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 13px;
    color: var(--ink-2);
  }

  .pin-on input {
    max-width: 320px;
  }

  .mins {
    width: 90px;
  }

  .note {
    font-size: 12.5px;
    color: var(--mut);
  }

  .key {
    list-style: none;
    margin: 0;
    padding: 12px;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(6.5rem, 1fr));
    gap: 0.4rem 1rem;
    border: 2px dashed var(--line-strong);
    border-radius: 8px;
  }

  .key li {
    display: flex;
    align-items: baseline;
    gap: 0.4rem;
  }

  .num {
    color: var(--mut);
    font-size: 0.8rem;
    min-width: 1.4rem;
    text-align: right;
  }

  code {
    font-family: var(--mono);
    font-size: 1.15rem;
    letter-spacing: 0.08em;
  }
</style>
