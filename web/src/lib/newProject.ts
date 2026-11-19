// "+ New project…" in every project picker (round 21): ask for a name, make the project and
// hand it back so the picker can choose it straight away.

import { api } from './api';
import type { Project } from './api';
import { askText } from './askText.svelte';
import type { MenuChoice } from './menu.svelte';
import { attempt, refreshProjects } from './store.svelte';

/** What the option says, the same in every picker. */
export const NEW_PROJECT = '+ New project…';

/** The value a `<select>` gives when that option is picked (never a project's id). */
export const NEW_PROJECT_VALUE = 'new-project';

/** Ask for a name and make the project; null when the box is closed or it can't be made. */
export async function newProject(): Promise<Project | null> {
  const name = (await askText({ title: 'New project', label: 'Project name', ok: 'Add project' }))?.trim();
  if (!name) return null;
  const made = await attempt(() => api.createProject(name, null));
  if (!made) return null;
  await attempt(refreshProjects);
  return made;
}

/** The last row of a menu's project choices: makes a project, then `pick`s it. */
export function newProjectChoice(pick: (id: string) => void): MenuChoice {
  return {
    label: NEW_PROJECT,
    color: null,
    checked: false,
    act: () =>
      void newProject().then((made) => {
        if (made) pick(made.id);
      }),
  };
}
