// Encrypted projects: Settings says whether the project is encrypted, a passphrase is put on it only after a warning window, the file
// on disk is then unreadable (read here as bytes), the project asks for its passphrase when it is opened again (a wrong one is told),
// the passphrase can be changed, and encryption can be turned off. A project can also be made encrypted from the New Project form.
// The check leaves the project it found: plain, and open.

import fs from 'node:fs';
import path from 'node:path';

const PASS = 'orange kettle window seven';
const PASS2 = 'a different passphrase entirely';

export async function runEncryption(ctx, api) {
  const { h, rpc, workDir } = ctx;
  const { page, S, axe } = api;

  const text = async (locator) => (await locator.innerText()).replace(/\s+/g, ' ').trim();
  const info = () => rpc('project.info');
  const header = (file) => fs.readFileSync(file).subarray(0, 15).toString('latin1');
  const holds = (file, needle) => fs.readFileSync(file).includes(Buffer.from(needle, 'utf8'));
  const openProtectionSettings = async () => {
    await page.getByRole('banner').getByRole('button', { name: 'Settings', exact: true }).click();
    await page.getByRole('heading', { level: 2, name: 'Settings' }).waitFor();
    await page.getByRole('navigation', { name: 'Settings sections' }).getByRole('button', { name: 'Privacy & Security' }).click();
    await page.getByTestId('encryption-panel').waitFor();
  };
  const until = async (ok, what) => {
    for (let i = 0; i < 80; i++) {
      if (await ok()) return;
      await page.waitForTimeout(100);
    }
    h.check(false, `waited for ${what}`);
  };
  /** Closes the project and gets back to the welcome page, whichever page of the launcher was left open. */
  const closeToWelcome = async () => {
    await page.getByTestId('project-name').click();
    await page.getByRole('menuitem', { name: 'Close project' }).click();
    await page.getByText('Closed the project.').first().waitFor();
    const welcome = page.getByRole('heading', { level: 2, name: 'Welcome to PrismQDA' });
    if (!(await welcome.isVisible().catch(() => false))) await page.getByRole('button', { name: 'Home', exact: true }).first().click();
    await welcome.waitFor();
  };
  /** Opens a project by typing its path in the Open Project window. */
  const openByPath = async (target) => {
    await page.getByRole('button', { name: 'Open Project' }).first().click();
    const dialog = page.getByRole('dialog', { name: 'Open Project' });
    await dialog.waitFor();
    await dialog.getByRole('textbox', { name: 'Project file' }).fill(target);
    await dialog.getByRole('button', { name: 'Open Project' }).click();
  };
  let file;
  let name;

  await S('Settings says the project is not encrypted and offers to encrypt it', async () => {
    const i = await info();
    file = i.path;
    name = i.name;
    h.equal(i.encrypted, false, 'the project starts plain');
    await openProtectionSettings();
    h.equal(await text(page.getByTestId('encryption-state')), 'This project is not encrypted.', 'the panel says so');
    h.equal(await page.getByTestId('encryption-start').count(), 1, 'and offers to encrypt');
    h.equal(await page.getByTestId('encrypted-badge').count(), 0, 'the status bar shows no badge');
    await axe('Settings, Privacy and Security, with a project that is not encrypted');
  });

  await S('a passphrase is checked in words, and the warning window comes before anything changes', async () => {
    // A backup made now is plain, and will be named afterwards.
    await rpc('project.backup');
    await page.getByTestId('encryption-start').click();
    await page.getByTestId('enc-passphrase').fill('short');
    await page.getByTestId('enc-again').fill('short');
    await page.getByTestId('enc-go').click();
    h.check(/at least 8 characters/.test(await text(page.getByTestId('encryption-panel').getByRole('alert').first())), 'a short passphrase is refused in a sentence');
    await page.getByTestId('enc-passphrase').fill(PASS);
    await page.getByTestId('enc-again').fill(`${PASS} typo`);
    await page.getByTestId('enc-go').click();
    h.check(/not the same/.test(await text(page.getByTestId('encryption-panel').getByRole('alert').first())), 'two passphrases that differ are told');
    h.equal((await info()).encrypted, false, 'nothing was changed');
    await page.getByTestId('enc-again').fill(PASS);
    await page.getByTestId('enc-go').click();
    const warning = page.getByTestId('lost-passphrase-warning');
    await warning.waitFor();
    const said = await text(warning);
    h.check(/nobody can ever open this project again/.test(said) && /not the author of this program/.test(said), `the warning says, in plain words, that nobody can help: ${said.slice(0, 160)}`);
    await axe('the warning window before a passphrase is put on a project');
    await page.getByTestId('lost-passphrase-cancel').click();
    await warning.waitFor({ state: 'detached' });
    h.equal((await info()).encrypted, false, 'cancelling leaves the project as it was');
    h.equal(header(file), 'SQLite format 3', 'and the file is still an ordinary one');
  });

  await S('after the warning, the project is encrypted: the file cannot be read, the old backups are named and can be deleted', async () => {
    await page.getByTestId('enc-go').click();
    await page.getByTestId('lost-passphrase-confirm').click();
    await page.getByTestId('enc-outcome').waitFor({ timeout: 20000 });
    h.check(/encrypted with AES-256-CBC/.test(await text(page.getByTestId('enc-outcome'))), 'the panel says what the encryption is');
    h.equal((await info()).encrypted, true, 'the engine says it is encrypted');
    h.equal(await text(page.getByTestId('encryption-state')), 'This project is encrypted.', 'and so does the panel');
    h.equal(await page.getByTestId('encrypted-badge').count(), 1, 'the status bar shows a badge');
    h.check(header(file) !== 'SQLite format 3', 'the file no longer begins like a SQLite file');
    h.check(!holds(file, name), `the file holds no word of what it holds: the project's name ${name} is not in it`);
    const walFile = `${file}-wal`;
    h.check(!fs.existsSync(walFile) || !holds(walFile, name), 'nor does the write-ahead file');
    const stale = page.getByTestId('enc-stale');
    await stale.waitFor();
    h.check(/not protected by this passphrase/.test(await text(stale)), 'the backups made before are said to be unprotected');
    await axe('Settings, Privacy and Security, with an encrypted project');
    await page.getByTestId('enc-delete-stale').click();
    await until(async () => /Deleted \d+ backup/.test(await text(page.getByTestId('enc-stale'))), 'the old backups to be deleted');
    h.equal((await rpc('project.stale_backups')).backups.length, 0, 'none is left');
  });

  await S('the project asks for its passphrase when it is opened again, and a wrong one is told', async () => {
    await closeToWelcome();
    // A project in the recent list says it is encrypted (the list asks the file system, not the project).
    const row = page.getByRole('button', { name: new RegExp(`^Open ${path.basename(file).replace(/[.]/g, '\\.')}`) });
    if ((await row.count()) > 0) h.check(/Encrypted/.test(await row.first().getAttribute('aria-label')), 'the recent list says the project is encrypted');
    await openByPath(file);
    const dialog = page.getByTestId('passphrase-dialog');
    await dialog.waitFor();
    h.check(/Type the passphrase of/.test(await text(dialog)), 'a window asks for the passphrase');
    await axe('the window that asks for the passphrase of an encrypted project');
    await page.getByTestId('open-passphrase').fill('not the right passphrase');
    await page.getByTestId('open-passphrase-submit').click();
    await dialog.getByRole('alert').waitFor();
    h.check(/does not open this project/.test(await text(dialog.getByRole('alert'))), 'a wrong passphrase is told');
    h.equal(await page.getByTestId('open-passphrase').inputValue(), '', 'and the field is emptied');
    await page.getByTestId('open-passphrase').fill(PASS);
    await page.getByTestId('open-passphrase-submit').click();
    await page.getByTestId('encrypted-badge').waitFor({ timeout: 20000 });
    h.equal((await info()).encrypted, true, 'the right passphrase opens it');
  });

  await S('the passphrase can be changed, and then only the new one opens the project', async () => {
    await openProtectionSettings();
    await page.getByTestId('encryption-change').click();
    await page.getByTestId('enc-old').fill('wrong wrong wrong');
    await page.getByTestId('enc-passphrase').fill(PASS2);
    await page.getByTestId('enc-again').fill(PASS2);
    await page.getByTestId('enc-go').click();
    await page.getByTestId('lost-passphrase-confirm').click();
    const refused = page.getByTestId('encryption-panel').getByRole('alert').filter({ hasText: /not the one this project has/ });
    await refused.waitFor();
    h.check((await refused.count()) === 1, 'a wrong current passphrase is told, and nothing is changed');
    await page.getByTestId('enc-old').fill(PASS);
    await page.getByTestId('enc-go').click();
    await page.getByTestId('lost-passphrase-confirm').click();
    await page.getByTestId('enc-outcome').waitFor({ timeout: 20000 });
    await closeToWelcome();
    await openByPath(file);
    await page.getByTestId('passphrase-dialog').waitFor();
    await page.getByTestId('open-passphrase').fill(PASS);
    await page.getByTestId('open-passphrase-submit').click();
    await page.getByTestId('passphrase-dialog').getByRole('alert').waitFor();
    h.check(true, 'the old passphrase no longer opens it');
    await page.getByTestId('open-passphrase').fill(PASS2);
    await page.getByTestId('open-passphrase-submit').click();
    await page.getByTestId('encrypted-badge').waitFor({ timeout: 20000 });
    h.check(true, 'the new one does');
  });

  await S('encryption is turned off with the passphrase, and the file is an ordinary one again', async () => {
    await openProtectionSettings();
    await page.getByTestId('encryption-remove').click();
    await page.getByTestId('enc-old').fill(PASS);
    await page.getByTestId('enc-go').click();
    const refused = page.getByTestId('encryption-panel').getByRole('alert').filter({ hasText: /not the one this project has/ });
    await refused.waitFor();
    h.check((await refused.count()) === 1, 'the wrong passphrase does not turn it off');
    h.equal((await info()).encrypted, true, 'and the project is still encrypted');
    await page.getByTestId('enc-old').fill(PASS2);
    await page.getByTestId('enc-go').click();
    await page.getByTestId('enc-outcome').waitFor({ timeout: 20000 });
    h.check(/no longer encrypted/.test(await text(page.getByTestId('enc-outcome'))), 'the panel says so');
    h.equal((await info()).encrypted, false, 'the engine agrees');
    h.equal(header(file), 'SQLite format 3', 'the file is an ordinary SQLite file again');
    h.equal(await page.getByTestId('encrypted-badge').count(), 0, 'and the badge is gone');
  });

  await S('a project can be encrypted from the start, in the New Project form, with the same warning', async () => {
    const made = path.join(workDir, 'Encrypted from the start.pqda');
    await closeToWelcome();
    await page.getByRole('button', { name: /^New Project/ }).first().click();
    await page.getByRole('textbox', { name: 'Project name' }).fill('Encrypted from the start');
    await page.getByRole('textbox', { name: 'Save location' }).fill(workDir);
    await page.getByTestId('np-encrypt').click();
    await page.getByTestId('np-passphrase').fill(PASS);
    await page.getByTestId('np-again').fill(PASS);
    await page.getByRole('button', { name: 'Create Project' }).click();
    await page.getByTestId('lost-passphrase-warning').waitFor();
    await page.getByTestId('lost-passphrase-confirm').click();
    await page.getByTestId('encrypted-badge').waitFor({ timeout: 20000 });
    h.equal((await info()).encrypted, true, 'the new project is encrypted');
    h.check(fs.existsSync(made) && header(made) !== 'SQLite format 3', 'and its file is unreadable as it is made');
    // Leave the check's project as it was found, open.
    await closeToWelcome();
    await openByPath(file);
    await page.getByTestId('project-name').filter({ hasText: name }).waitFor({ timeout: 20000 });
    h.equal((await info()).path, file, 'the original project is open again');
    h.equal(await page.getByTestId('encrypted-badge').count(), 0, 'and it is plain, as it was found');
  });
}
