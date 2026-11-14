import { test, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { setTransportForTests } from '../bridge';
import { noticeStore } from './announce';
import { appStore } from './app';
import { openLaunchedProject, samePath, takeLaunchPath, watchLaunchPaths } from './launch';

const g = globalThis as unknown as Record<string, unknown>;

afterEach(() => {
  setTransportForTests(null);
  delete g.window;
  mock.timers.reset();
});

/** What the window last said, read from the list of messages (the status line itself fills in a moment later). The timers are held so the status line's fade does not keep the test waiting. */
function lastSaid(): string {
  return noticeStore.get().items[0]?.text ?? '';
}

function info(name: string, path: string) {
  return { name, path, project_uid: `uid-${name}`, sources: 0, codes: 0, coders: 0, codings: 0, events: 0, notices: [], me: 1, read_only: false, coding_mode: 'free_span' };
}

test('two spellings of one Windows path are the same file, and two files are not', () => {
  assert.equal(samePath('C:\\Studies\\One.pqda', 'c:\\studies\\one.PQDA'), true, 'letters in either case');
  assert.equal(samePath('C:/Studies/one.pqda', 'C:\\Studies\\one.pqda'), true, 'either kind of slash');
  assert.equal(samePath('\\\\?\\C:\\Studies\\one.pqda', 'C:\\Studies\\one.pqda'), true, 'the long form Windows sometimes gives');
  assert.equal(samePath('C:\\Studies\\one.pqda', 'C:\\Studies\\two.pqda'), false);
  assert.equal(samePath('C:\\Studies\\one.pqda', 'D:\\Studies\\one.pqda'), false);
});

test('outside the Windows program there is no launch path and nothing is listened for', async () => {
  g.window = {};
  assert.equal(await takeLaunchPath(), null);
  const stop = await watchLaunchPaths();
  assert.equal(typeof stop, 'function');
  stop();
});

test('inside the program the launch path is asked for with the launch_path command', async () => {
  const asked: string[] = [];
  let answer: unknown = 'C:\\Studies\\one.pqda';
  g.window = {
    __TAURI_INTERNALS__: {
      invoke: async (name: string) => {
        asked.push(name);
        return answer;
      },
    },
  };
  assert.equal(await takeLaunchPath(), 'C:\\Studies\\one.pqda');
  answer = null;
  assert.equal(await takeLaunchPath(), null, 'nothing was sent');
  answer = '';
  assert.equal(await takeLaunchPath(), null, 'an empty answer is nothing too');
  assert.deepEqual(asked, ['launch_path', 'launch_path', 'launch_path']);
  g.window = { __TAURI_INTERNALS__: { invoke: async () => { throw new Error('gone'); } } };
  assert.equal(await takeLaunchPath(), null, 'a failure is not a path');
});

test('a project that is already open is not opened again, and the window says so', async () => {
  mock.timers.enable({ apis: ['setTimeout'] });
  const calls: string[] = [];
  setTransportForTests(async (cmd) => {
    calls.push(cmd);
    return [];
  });
  appStore.set({ project: info('Study', 'C:\\Studies\\Study.pqda') as never });
  await openLaunchedProject('c:\\studies\\STUDY.pqda');
  assert.deepEqual(calls, [], 'the engine was not asked to open anything');
  assert.match(lastSaid(), /Study is already open/);
  appStore.set({ project: null });
});

test('a double clicked project that cannot be opened is said in words and the window stays where it was', async () => {
  mock.timers.enable({ apis: ['setTimeout'] });
  setTransportForTests(async (cmd) => {
    if (cmd === 'project.open') throw { code: 'not_a_project', message: 'that file is not a PrismQDA project' };
    return [];
  });
  appStore.set({ project: null, phase: 'start' });
  await openLaunchedProject('C:\\Studies\\broken.pqda');
  const said = lastSaid();
  assert.match(said, /broken\.pqda could not be opened/);
  assert.match(said, /not a PrismQDA project/);
  assert.equal(appStore.get().phase, 'start');
});

test('an encrypted project asks for its passphrase instead of failing', async () => {
  setTransportForTests(async (cmd) => {
    if (cmd === 'project.open') throw { code: 'needs_passphrase', message: 'this project is encrypted' };
    return [];
  });
  appStore.set({ project: null, phase: 'start', dialog: null });
  await openLaunchedProject('C:\\Studies\\secret.pqda');
  assert.deepEqual(appStore.get().dialog, { kind: 'passphrase', path: 'C:\\Studies\\secret.pqda' });
  appStore.set({ dialog: null });
});

test('a double clicked project is opened and the workspace is shown', async () => {
  setTransportForTests(async (cmd) => {
    if (cmd === 'project.open') return info('Other', 'C:\\Studies\\Other.pqda');
    return [];
  });
  appStore.set({ project: info('Study', 'C:\\Studies\\Study.pqda') as never, phase: 'workspace' });
  await openLaunchedProject('C:\\Studies\\Other.pqda');
  assert.equal(appStore.get().project?.name, 'Other');
  assert.equal(appStore.get().phase, 'workspace');
  assert.equal(appStore.get().recent[0]?.path, 'C:\\Studies\\Other.pqda', 'and it is at the top of the recent list');
});
