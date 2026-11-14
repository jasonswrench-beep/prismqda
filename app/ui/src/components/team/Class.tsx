import { useMemo, useState } from 'react';
import { Button, Input, Label, Text, TextArea, TextField } from 'react-aria-components';
import { call, toApiError } from '../../bridge';
import { kappaText, parseStudents, percentText, suggestedClassFolder, suggestedReviewFile } from '../../lib/classroom';
import { browsePath, cleanTypedPath, nativeDialogsAvailable } from '../../pickFile';
import { announce } from '../../state/announce';
import { refreshEverything } from '../../state/actions';
import { useApp } from '../../state/app';
import type { ClassCollectReport, ClassPacketsReport, ClassReview, ClassReviewArgs, SourceRow } from '../../types';
import { Notice } from '../common';
import { Tick } from '../Tick';
import { num, t, tn } from '../../i18n';

/** The documents of the project as ticks. None ticked means all of them, which the words under the list say. */
function DocumentTicks(props: { legend: string; picked: number[]; setPicked: (ids: number[]) => void; sources: SourceRow[]; testId: string }) {
  const { legend, picked, setPicked, sources, testId } = props;
  return (
    <fieldset className="field team-coders class-docs" data-testid={testId}>
      <legend>{legend}</legend>
      {sources.map((s) => (
        <Tick key={s.id} isSelected={picked.includes(s.id)} onChange={(on) => setPicked(on ? [...picked, s.id] : picked.filter((x) => x !== s.id))} testId={`${testId}-${s.id}`}>{s.name}</Tick>
      ))}
      <p className="help">{picked.length === 0 ? t('None is ticked, so every document is used.') : tn(picked.length, { one: '{count} document is ticked.', other: '{count} documents are ticked.' })}</p>
    </fieldset>
  );
}

/**
 * Class: an exercise for a class (decision 183). The instructor codes the documents, makes a blind packet for every student, brings the
 * packets back in one step, and reviews the class against their own coding: who found what, which codes were hard, and which passages the
 * class coded differently. The packets, Merge and the agreement measures are the ones the other tabs use.
 */
export function ClassTab() {
  const project = useApp((s) => s.project);
  const sources = useApp((s) => s.sources);
  const coders = useApp((s) => s.coders);
  const readOnly = project?.read_only ?? false;

  const [folder, setFolder] = useState('');
  const fallback = suggestedClassFolder(project?.path ?? '');
  const where = () => cleanTypedPath(folder) || fallback;
  const chooseFolder = async () => {
    const picked = await browsePath('class-folder');
    if (picked) setFolder(picked);
  };

  // ---- one packet for each student ----
  const [list, setList] = useState('');
  const [packetDocs, setPacketDocs] = useState<number[]>([]);
  const [instructions, setInstructions] = useState('');
  const [making, setMaking] = useState(false);
  const [makeError, setMakeError] = useState<string | null>(null);
  const [made, setMade] = useState<ClassPacketsReport | null>(null);
  const parsed = useMemo(() => parseStudents(list), [list]);

  const make = async () => {
    setMakeError(null);
    setMade(null);
    if (parsed.names.length === 0) return setMakeError(t('Type the names of the students, one on each line.'));
    setMaking(true);
    try {
      const r = await call('class.packets', { folder: where(), students: parsed.names, source_ids: packetDocs, instructions });
      setMade(r);
      announce(tn(r.made, { one: '{count} packet was made.', other: '{count} packets were made.' }));
    } catch (e) {
      setMakeError(toApiError(e).message);
    } finally {
      setMaking(false);
    }
  };

  // ---- bringing the packets back ----
  const [collecting, setCollecting] = useState(false);
  const [collectError, setCollectError] = useState<string | null>(null);
  const [collected, setCollected] = useState<ClassCollectReport | null>(null);
  const collect = async () => {
    setCollectError(null);
    setCollected(null);
    setCollecting(true);
    try {
      const r = await call('class.collect', { folder: where() });
      setCollected(r);
      await refreshEverything();
      announce(tn(r.packets, { one: '{count} student’s work was brought in.', other: '{count} students’ work was brought in.' }));
    } catch (e) {
      setCollectError(toApiError(e).message);
    } finally {
      setCollecting(false);
    }
  };

  // ---- the review ----
  const humans = coders.filter((c) => c.role === 'human');
  const [instructorPick, setInstructorPick] = useState<number | null>(null);
  const instructor = instructorPick ?? project?.me ?? humans[0]?.id ?? null;
  const others = coders.filter((c) => c.id !== instructor);
  const [studentPick, setStudentPick] = useState<number[] | null>(null);
  // Until a choice is made, every other person who codes is a student; a model is a student only on purpose.
  const students = (studentPick ?? others.filter((c) => c.role === 'human').map((c) => c.id)).filter((id) => id !== instructor);
  const [reviewDocs, setReviewDocs] = useState<number[]>([]);
  const [anonymous, setAnonymous] = useState(false);
  const [hardest, setHardest] = useState('10');
  const [reviewing, setReviewing] = useState(false);
  const [reviewError, setReviewError] = useState<string | null>(null);
  const [review, setReview] = useState<ClassReview | null>(null);
  const [savePath, setSavePath] = useState('');
  const [saved, setSaved] = useState<string | null>(null);

  const reviewArgs = (): ClassReviewArgs => ({ ...(instructor !== null ? { instructor_id: instructor } : {}), student_ids: students, source_ids: reviewDocs, anonymous, hardest: Number(hardest) });
  const run = async () => {
    setReviewError(null);
    setSaved(null);
    if (instructor === null) return setReviewError(t('Choose the instructor, whose coding the class is compared with.'));
    if (students.length === 0) return setReviewError(t('Choose at least one student. A student is a coder other than the instructor; bring the packets in first.'));
    setReviewing(true);
    try {
      const r = await call('class.review', reviewArgs());
      setReview(r);
      announce(tn(r.students.length, { one: 'The review of {count} student is ready.', other: 'The review of {count} students is ready.' }));
    } catch (e) {
      setReview(null);
      setReviewError(toApiError(e).message);
    } finally {
      setReviewing(false);
    }
  };
  const save = async () => {
    setReviewError(null);
    setSaved(null);
    if (instructor === null || students.length === 0) return setReviewError(t('Choose the instructor and at least one student first.'));
    const suggestion = suggestedReviewFile(project?.name ?? '');
    let target = cleanTypedPath(savePath);
    let fromDialog = false;
    if (target === '') {
      const picked = await browsePath('export-file', suggestion, 'csv');
      if (!picked) return setReviewError(t('Type where to save the table, for example C:\\Class\\review.csv'));
      target = picked;
      fromDialog = true;
    }
    try {
      const r = await call('export.class_review', { ...reviewArgs(), path: target, overwrite: fromDialog });
      setSaved(r.path);
      announce(t('Saved {path}.', { path: r.path }));
    } catch (e) {
      setReviewError(toApiError(e).message);
    }
  };

  return (
    <div className="class-tab" data-testid="class-tab">
      <section className="ac-card" aria-labelledby="h-class-folder">
        <h3 id="h-class-folder" className="bk-h3">{t('A class exercise')}</h3>
        <p className="help">{t('Code the documents of the exercise yourself first; your coding is the answer the class is compared with. Make a packet for each student, send each student their own file, and put the files they send back in one folder. One step brings all their work in, and the review shows how the class did against yours. A student never sees your coding or anyone else’s.')}</p>
        <TextField value={folder} onChange={setFolder} className="field">
          <Label>{t('The class folder')}</Label>
          <div className="bk-location-row">
            <Input placeholder={fallback} data-testid="class-folder" />
            {nativeDialogsAvailable ? <Button type="button" aria-label={t('Browse for the class folder')} onPress={() => void chooseFolder()}>{t('Browse…')}</Button> : null}
          </div>
          <Text slot="description">{t('The packets are made in this folder (it is made when it is not there), and the students’ files are brought in from it. Leave it empty to use a folder beside this project.')}</Text>
        </TextField>
      </section>

      <section className="ac-card" aria-labelledby="h-class-packets" data-testid="class-packets-card">
        <h3 id="h-class-packets" className="bk-h3">{t('1. Make a packet for every student')}</h3>
        {makeError ? <Notice>{makeError}</Notice> : null}
        <TextField value={list} onChange={(v) => { setList(v); setMade(null); }} className="field">
          <Label>{t('The students, one name on each line')}</Label>
          <TextArea rows={6} data-testid="class-students" />
          <Text slot="description">{parsed.names.length === 0 ? t('Paste the names from your roster.') : tn(parsed.names.length, { one: '{count} student.', other: '{count} students.' }) + (parsed.repeated.length > 0 ? ' ' + t('A name that came twice is used once: {names}.', { names: parsed.repeated.join(', ') }) : '')}</Text>
        </TextField>
        <DocumentTicks legend={t('The documents to put in the packets')} picked={packetDocs} setPicked={setPacketDocs} sources={sources} testId="class-packet-docs" />
        <TextField value={instructions} onChange={setInstructions} className="field">
          <Label>{t('Instructions for the students')}</Label>
          <TextArea rows={3} data-testid="class-instructions" />
          <Text slot="description">{t('Each student sees this as a memo in their packet: which codes to use, how many to give a passage, when to send it back.')}</Text>
        </TextField>
        <div className="actions">
          <Button className="primary" onPress={() => void make()} isDisabled={making} data-testid="class-make">{making ? t('Making…') : t('Make the packets')}</Button>
        </div>
        {made ? (
          <div data-testid="class-packets-result">
            <Notice kind="info">{tn(made.made, { one: '{count} packet was made in {folder}. Send each student their own file.', other: '{count} packets were made in {folder}. Send each student their own file.' }, { folder: made.folder })}</Notice>
            {made.repeated.length > 0 ? <p className="muted">{t('Left out because the name came twice: {names}.', { names: made.repeated.join(', ') })}</p> : null}
            <div className="table-wrap" tabIndex={-1}>
              <table className="data" data-testid="class-packet-table">
                <caption>{t('The packets')}</caption>
                <thead>
                  <tr>
                    <th scope="col">{t('Student')}</th>
                    <th scope="col">{t('File')}</th>
                    <th scope="col">{t('Result')}</th>
                  </tr>
                </thead>
                <tbody>
                  {made.packets.map((p) => (
                    <tr key={p.path} data-testid="class-packet-row">
                      <th scope="row">{p.student}</th>
                      <td>{p.path.split(/[\\/]/).pop()}</td>
                      <td>{p.made ? t('Made.') : (p.error ?? t('Not made.'))}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <p className="muted">{t('Each packet holds the codebook, {documents}, and your instructions, and none of your coding. The packets are not encrypted.', { documents: made.documents.length === 1 ? made.documents[0] : tn(made.documents.length, { one: '{count} document', other: '{count} documents' }) })}</p>
          </div>
        ) : null}
      </section>

      <section className="ac-card" aria-labelledby="h-class-collect" data-testid="class-collect-card">
        <h3 id="h-class-collect" className="bk-h3">{t('2. Bring the students’ work in')}</h3>
        <p className="help">{t('Every packet of this project in the class folder is brought in: each student’s coding stays with the student as a coder. A backup of this project is made first. A file that is not a packet, or that holds documents this project does not have, is left where it is and listed. Bringing the same files in again changes nothing.')}</p>
        {collectError ? <Notice>{collectError}</Notice> : null}
        <div className="actions">
          <Button className="primary" onPress={() => void collect()} isDisabled={collecting || readOnly} data-testid="class-collect">{collecting ? t('Bringing in…') : t('Bring in the students’ work')}</Button>
        </div>
        {collected ? (
          <div data-testid="class-collect-result">
            <Notice kind="info">
              {collected.packets === 0
                ? t('No packet was found in {folder}.', { folder: collected.folder })
                : tn(collected.codings_added, { one: '{count} coding was brought in from {packets} packets.', other: '{count} codings were brought in from {packets} packets.' }, { packets: num(collected.packets) })}
              {collected.backup ? ' ' + t('A backup was saved first: {path}.', { path: collected.backup }) : ''}
            </Notice>
            {collected.files.length > 0 ? (
              <div className="table-wrap" tabIndex={-1}>
                <table className="data" data-testid="class-collect-table">
                  <caption>{t('The files in the folder')}</caption>
                  <thead>
                    <tr>
                      <th scope="col">{t('File')}</th>
                      <th scope="col">{t('Student')}</th>
                      <th scope="col">{t('Result')}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {collected.files.map((f) => (
                      <tr key={f.file} data-testid="class-collect-row" data-status={f.status}>
                        <th scope="row">{f.file}</th>
                        <td>{f.student ?? ''}</td>
                        <td>{f.message}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            ) : null}
          </div>
        ) : null}
      </section>

      <section className="ac-card" aria-labelledby="h-class-review" data-testid="class-review-card">
        <h3 id="h-class-review" className="bk-h3">{t('3. Review the class against your coding')}</h3>
        <p className="help">{t('Each student is compared with the instructor passage by passage: did they give the same codes, some of them, others, or none. Only text is compared; pictures, PDF areas and recordings are not.')}</p>
        {reviewError ? <Notice>{reviewError}</Notice> : null}
        <div className="field">
          <label htmlFor="class-instructor">{t('The instructor, whose coding is the answer')}</label>
          <select id="class-instructor" value={instructor ?? ''} onChange={(e) => { setInstructorPick(Number(e.target.value)); setStudentPick(null); }} data-testid="class-instructor">
            {coders.map((c) => (
              <option key={c.id} value={c.id}>{c.role === 'model' ? t('{name} (AI model)', { name: c.name }) : c.name}</option>
            ))}
          </select>
        </div>
        <fieldset className="field team-coders class-students" data-testid="class-student-ticks">
          <legend>{t('The students')}</legend>
          {others.length === 0 ? <p className="muted">{t('There is no other coder yet. Bring the students’ work in first.')}</p> : null}
          {others.map((c) => (
            <Tick key={c.id} isSelected={students.includes(c.id)} onChange={(on) => setStudentPick(on ? [...students, c.id] : students.filter((x) => x !== c.id))} testId={`class-student-${c.id}`}>{c.role === 'model' ? t('{name} (AI model)', { name: c.name }) : c.name}</Tick>
          ))}
        </fieldset>
        <DocumentTicks legend={t('The documents to compare on')} picked={reviewDocs} setPicked={setReviewDocs} sources={sources} testId="class-review-docs" />
        <Tick isSelected={anonymous} onChange={(on) => { setAnonymous(on); setReview(null); setSaved(null); }} testId="class-anonymous">{t('Call the students “Student 1”, “Student 2” and so on, so the review can be shown to the class (the lists of students above it on this page still show their names)')}</Tick>
        <div className="field">
          <label htmlFor="class-hardest">{t('How many of the hardest passages to list')}</label>
          <select id="class-hardest" value={hardest} onChange={(e) => setHardest(e.target.value)} data-testid="class-hardest">
            {['5', '10', '20', '50'].map((n) => (
              <option key={n} value={n}>{n}</option>
            ))}
          </select>
        </div>
        <div className="actions">
          <Button className="primary" onPress={() => void run()} isDisabled={reviewing} data-testid="class-review">{reviewing ? t('Reviewing…') : t('Review the class')}</Button>
        </div>

        {review ? (
          <div data-testid="class-review-result">
            <h4 className="bk-h4">{tn(review.students.length, { one: '{count} student compared with {instructor} on {passages} passages', other: '{count} students compared with {instructor} on {passages} passages' }, { instructor: review.instructor.name, passages: num(review.instructor_passages) })}</h4>
            <ul className="class-notes" data-testid="class-notes">
              {review.notes.map((n) => (
                <li key={n}>{n}</li>
              ))}
            </ul>
            <div className="table-wrap" tabIndex={-1}>
              <table className="data" data-testid="class-students-table">
                <caption>{t('How each student’s coding compares with the instructor’s')}</caption>
                <thead>
                  <tr>
                    <th scope="col">{t('Student')}</th>
                    <th scope="col">{t('Passages coded')}</th>
                    <th scope="col">{t('Same codes')}</th>
                    <th scope="col">{t('Some of the same')}</th>
                    <th scope="col">{t('Other codes only')}</th>
                    <th scope="col">{t('Left uncoded')}</th>
                    <th scope="col">{t('Coded, instructor did not')}</th>
                    <th scope="col">{t('Passages the same')}</th>
                    <th scope="col">{t('Codes found')}</th>
                  </tr>
                </thead>
                <tbody>
                  {review.students.map((s) => (
                    <tr key={s.label} data-testid="class-student-row">
                      <th scope="row">{s.label}</th>
                      <td>{num(s.passages_coded)}</td>
                      <td>{num(s.same)}</td>
                      <td>{num(s.partly)}</td>
                      <td>{num(s.different)}</td>
                      <td>{num(s.left_uncoded)}</td>
                      <td>{num(s.extra)}</td>
                      <td>{percentText(s.share_same)}</td>
                      <td>{percentText(s.share_found)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <p className="help">{t('“Passages the same” is the share of the instructor’s passages where the student gave exactly the same codes. “Codes found” is the share of the codes the instructor gave that the student gave too.')}</p>

            <div className="table-wrap" tabIndex={-1}>
              <table className="data" data-testid="class-codes-table">
                <caption>{t('How the class did on each code, the hardest first')}</caption>
                <thead>
                  <tr>
                    <th scope="col">{t('Code')}</th>
                    <th scope="col">{t('Instructor’s passages')}</th>
                    <th scope="col">{t('Found, on average')}</th>
                    <th scope="col">{t('Given where the instructor did not')}</th>
                    <th scope="col">{t('Kappa, on average')}</th>
                  </tr>
                </thead>
                <tbody>
                  {review.by_code.map((c) => (
                    <tr key={c.code_id} data-testid="class-code-row">
                      <th scope="row">{c.name}</th>
                      <td>{num(c.instructor_passages)}</td>
                      <td>{percentText(c.mean_found)}</td>
                      <td>{num(c.added)}</td>
                      <td>{kappaText(c.mean_kappa)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>

            {review.hardest.length > 0 ? (
              <>
                <h4 className="bk-h4" id="class-hardest-h">{t('The passages the class coded least like the instructor')}</h4>
                <ol className="class-hardest" aria-labelledby="class-hardest-h" data-testid="class-hardest-list">
                  {review.hardest.map((h) => (
                    <li key={h.unit_id} data-testid="class-hard-passage">
                      <p className="class-hard-where">{t('{document}: {same} of {total} students gave the instructor’s codes.', { document: h.source_name, same: num(h.students_same), total: num(h.students_total) })}</p>
                      <blockquote className="class-hard-text">{h.text}</blockquote>
                      <p>{t('The instructor gave: {codes}.', { codes: h.instructor_codes.map((c) => c.name).join(', ') })}</p>
                      {h.added_codes.length > 0 ? <p>{t('Students also gave: {codes}.', { codes: h.added_codes.map(([c, n]) => t('{code} ({count})', { code: c.name, count: num(n) })).join(', ') })}</p> : null}
                      {h.missed_codes.length > 0 ? <p>{t('Students left out: {codes}.', { codes: h.missed_codes.map(([c, n]) => t('{code} ({count})', { code: c.name, count: num(n) })).join(', ') })}</p> : null}
                    </li>
                  ))}
                </ol>
              </>
            ) : (
              <p className="muted" data-testid="class-no-hard">{t('Every student coded every passage the instructor coded in the same way.')}</p>
            )}

            <div className="field">
              <label htmlFor="class-save-path">{t('Save the table of students as a file')}</label>
              <div className="bk-location-row">
                <input id="class-save-path" type="text" value={savePath} onChange={(e) => setSavePath(e.target.value)} autoComplete="off" spellCheck={false} placeholder={t('C:\\Class\\review.csv')} data-testid="class-save-path" />
                <Button onPress={() => void save()} data-testid="class-save">{t('Save the table')}</Button>
              </div>
              <span className="help">{t('A CSV file with one row for each student, to open in a spreadsheet. A file that is already there is not replaced. The names in it are the students’ names, whatever list of names is applied to other exports.')}</span>
              {saved ? <p role="status" data-testid="class-saved">{t('Saved {path}.', { path: saved })}</p> : null}
            </div>
          </div>
        ) : null}
      </section>
    </div>
  );
}
