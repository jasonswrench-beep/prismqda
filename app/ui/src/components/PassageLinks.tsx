import { useCallback, useEffect, useMemo, useState } from 'react';
import { Button, Dialog, Heading, Input, Label, Modal, ModalOverlay, Radio, RadioGroup, Text, TextField } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { openAt } from '../state/actions';
import { announce } from '../state/announce';
import { appStore, useApp } from '../state/app';
import type { LinkRelations, PassageLink, SegmentRow } from '../types';
import { Icon } from './Icon';
import { SegmentQuote } from './SegmentQuote';
import { Notice } from './common';
import { t } from '../i18n';

/** The words a passage is shown by in a list: the quotation, or for an area or a stretch its words. */
function PassageButton(props: { seg: SegmentRow; label: string }) {
  const { seg } = props;
  return (
    <Button className="link-passage" aria-label={props.label} onPress={() => void openAt(seg.source_id, seg.start, seg.end, seg.coding_id)} data-testid="link-open">
      <SegmentQuote text={seg.text} sourceId={seg.source_id} region={seg.region} page={seg.page ?? null} max={160} />
      <span className="muted link-where">{`${seg.source_name} · ${seg.code_name}`}</span>
    </Button>
  );
}

/**
 * The links between passages that touch one passage (Segment Details), or all of them (the Synthesize page). Each says how the two are
 * related and shows both ends; either can be opened in its document. A link can be removed, and Undo takes the removal back (decision 162).
 */
function LinkList(props: { links: PassageLink[]; mine: number | null; readOnly: boolean; onRemoved: () => void }) {
  const remove = async (l: PassageLink) => {
    try {
      await call('link.delete', { id: l.id });
      announce(t('Removed the link “{relation}”.', { relation: l.relation }));
      props.onRemoved();
      // The button that was pressed goes with the link: the keyboard goes to the button that adds one, or to the page.
      window.setTimeout(() => (document.querySelector<HTMLElement>('[data-testid=link-add]') ?? document.getElementById('main-content'))?.focus(), 0);
    } catch (e) {
      announce(toApiError(e).message, 'assertive');
    }
  };
  return (
    <ul className="passage-links" data-testid="passage-links">
      {props.links.map((l) => {
        const outgoing = props.mine === null || l.from.coding_id === props.mine;
        const other = props.mine === null ? null : outgoing ? l.to : l.from;
        return (
          <li key={l.id} className="passage-link" data-testid="passage-link">
            {other ? (
              <>
                <p className="link-sentence">
                  {outgoing ? t('This passage {relation}', { relation: l.relation }) : t('{relation} this passage', { relation: l.relation })}
                </p>
                <PassageButton seg={other} label={outgoing ? t('Open the passage that this passage {relation}', { relation: l.relation }) : t('Open the passage that {relation} this passage', { relation: l.relation })} />
              </>
            ) : (
              <>
                <PassageButton seg={l.from} label={t('Open the first passage, which {relation} the second', { relation: l.relation })} />
                <p className="link-sentence" data-testid="link-relation">{`↓ ${l.relation}`}</p>
                <PassageButton seg={l.to} label={t('Open the second passage')} />
              </>
            )}
            {props.readOnly ? null : (
              <Button className="icon-button link-remove" aria-label={t('Remove the link “{relation}”', { relation: l.relation })} onPress={() => void remove(l)} data-testid="link-remove">
                <Icon name="ui-close" size={14} />
              </Button>
            )}
          </li>
        );
      })}
    </ul>
  );
}

/** In Segment Details: the links of the selected passage, and the button that links it to another. */
export function PassageLinksSection(props: { codingId: number }) {
  const readOnly = useApp((s) => s.project?.read_only ?? false);
  const version = useApp((s) => s.dataVersion);
  const [links, setLinks] = useState<PassageLink[] | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const load = useCallback(async () => {
    try {
      setLinks(await call('link.list', { coding_id: props.codingId }));
      setProblem(null);
    } catch (e) {
      setLinks([]);
      setProblem(toApiError(e).message);
    }
  }, [props.codingId]);
  useEffect(() => {
    setLinks(null);
    void load();
  }, [load, version]);
  return (
    <section className="bk-section" aria-labelledby="seg-links" data-testid="seg-links">
      <div className="bk-section-head">
        <h3 id="seg-links" className="bk-h3">{links ? t('Links ({count})', { count: links.length }) : t('Links')}</h3>
        {readOnly ? null : (
          <Button className="icon-button" aria-label={t('Link this passage to another passage')} onPress={() => appStore.set({ dialog: { kind: 'link-passage', codingId: props.codingId } })} data-testid="link-add">
            <Icon name="ui-plus" size={18} />
          </Button>
        )}
      </div>
      {problem ? <Notice>{problem}</Notice> : null}
      {links && links.length > 0 ? <LinkList links={links} mine={props.codingId} readOnly={readOnly} onRemoved={() => void load()} /> : problem ? null : <p className="muted">{links === null ? t('Looking for links…') : t('This passage is not linked to another yet.')}</p>}
    </section>
  );
}

/** On the Synthesize page: every link between passages. */
export function PassageLinksPage() {
  const readOnly = useApp((s) => s.project?.read_only ?? false);
  const version = useApp((s) => s.dataVersion);
  const [links, setLinks] = useState<PassageLink[] | null>(null);
  const [relations, setRelations] = useState<LinkRelations | null>(null);
  const [only, setOnly] = useState('');
  const load = useCallback(async () => {
    try {
      const [l, r] = await Promise.all([call('link.list', {}), call('link.relations', {})]);
      setLinks(l);
      setRelations(r);
    } catch {
      setLinks([]);
    }
  }, []);
  useEffect(() => {
    void load();
  }, [load, version]);
  // A relation that no link carries any more is not a filter any more.
  useEffect(() => {
    if (only !== '' && relations && !relations.used.some((r) => r.name.toLowerCase() === only.toLowerCase())) setOnly('');
  }, [only, relations]);
  const shown = links ? links.filter((l) => only === '' || l.relation.toLowerCase() === only.toLowerCase()) : [];
  return (
    <section className="ac-card" aria-labelledby="h-links-page" data-testid="links-page">
      <h3 id="h-links-page" className="bk-h3">{t('Links between passages')}</h3>
      <p className="help">{t('A link says how one coded passage relates to another: it contradicts it, explains it, leads to it. Make one from Segment Details: select a coded passage and press the plus beside Links.')}</p>
      {links === null ? <p className="muted">{t('Looking for links…')}</p> : null}
      {links && links.length === 0 ? <p className="muted" data-testid="links-empty">{t('No passage is linked to another yet.')}</p> : null}
      {links && links.length > 0 ? (
        <>
          <div className="field">
            <label htmlFor="links-relation">{t('Show')}</label>
            <select id="links-relation" value={only} onChange={(e) => setOnly(e.target.value)}>
              <option value="">{t('Every link ({count})', { count: links.length })}</option>
              {relations?.used.map((r) => (
                <option key={r.name} value={r.name}>{`${r.name} (${r.uses})`}</option>
              ))}
            </select>
          </div>
          <LinkList links={shown} mine={null} readOnly={readOnly} onRemoved={() => void load()} />
        </>
      ) : null}
    </section>
  );
}

/** Link the passage that is selected to another one the person finds and chooses, with a relation. */
export function LinkPassageDialog(props: { codingId: number }) {
  const [here, setHere] = useState<SegmentRow | null>(null);
  const [all, setAll] = useState<SegmentRow[] | null>(null);
  const [cut, setCut] = useState(false);
  const [relations, setRelations] = useState<LinkRelations | null>(null);
  const [search, setSearch] = useState('');
  const [chosen, setChosen] = useState<number | null>(null);
  const [relation, setRelation] = useState('');
  const [direction, setDirection] = useState<'out' | 'in'>('out');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const close = () => appStore.set({ dialog: null });

  useEffect(() => {
    let live = true;
    Promise.all([call('segments.list', { limit: 5000 }), call('segments.list', { coding_ids: [props.codingId] }), call('link.relations', {})])
      .then(([s, mine, r]) => {
        if (!live) return;
        setAll(s.segments);
        setCut(s.truncated);
        setHere(mine.segments[0] ?? null);
        setRelations(r);
      })
      .catch((e) => live && setError(toApiError(e).message));
    return () => {
      live = false;
    };
  }, [props.codingId]);

  const words = relations ? [...relations.used.map((r) => r.name), ...relations.common.filter((c) => !relations.used.some((r) => r.name.toLowerCase() === c.toLowerCase()))] : [];
  const needle = search.trim().toLowerCase();
  const candidates = useMemo(() => {
    if (!all) return [];
    return all
      .filter((s) => s.coding_id !== props.codingId)
      .filter((s) => needle === '' || `${s.text} ${s.code_name} ${s.source_name}`.toLowerCase().includes(needle))
      .slice(0, 40);
  }, [all, needle, props.codingId]);

  const save = async () => {
    setError(null);
    if (chosen === null) return setError(t('Choose the passage to link to.'));
    if (relation.trim() === '') return setError(t('Say how the two passages are related, in a word or two.'));
    setBusy(true);
    try {
      await call('link.add', direction === 'out' ? { from_coding: props.codingId, to_coding: chosen, relation } : { from_coding: chosen, to_coding: props.codingId, relation });
      announce(t('Linked the two passages: {relation}.', { relation: relation.trim() }));
      appStore.set((s) => ({ dialog: null, dataVersion: s.dataVersion + 1 }));
    } catch (e) {
      setError(toApiError(e).message);
      setBusy(false);
    }
  };

  return (
    <ModalOverlay isOpen isDismissable={false} onOpenChange={(o) => !o && close()} className="overlay">
      <Modal className="modal modal-wide">
        <Dialog className="dialog" aria-labelledby="link-title">
          <Heading slot="title" id="link-title" className="dialog-title">{t('Link this passage to another')}</Heading>
          {here ? (
            <div data-testid="link-here">
              <p className="muted">{t('This passage:')}</p>
              <SegmentQuote text={here.text} sourceId={here.source_id} region={here.region} page={here.page ?? null} max={200} />
            </div>
          ) : null}
          {error ? <Notice>{error}</Notice> : null}
          <RadioGroup value={direction} onChange={(v) => setDirection(v as 'out' | 'in')} className="field ac-radios" aria-label={t('Which way the relation runs')}>
            <Radio value="out" className="react-aria-Radio ac-radio" data-testid="link-dir-out"><span className="ac-radio-label">{t('This passage … the one I choose')}</span></Radio>
            <Radio value="in" className="react-aria-Radio ac-radio" data-testid="link-dir-in"><span className="ac-radio-label">{t('The one I choose … this passage')}</span></Radio>
          </RadioGroup>
          <TextField value={relation} onChange={setRelation} className="field">
            <Label>{t('How they are related')}</Label>
            <Input list="link-relations" maxLength={80} autoComplete="off" data-testid="link-relation-input" />
            <Text slot="description">{t('A word or two, such as “contradicts” or “explains”. The relations you used before and some common ones are offered; any words will do.')}</Text>
          </TextField>
          <datalist id="link-relations">
            {words.map((w) => (
              <option key={w} value={w} />
            ))}
          </datalist>
          <TextField value={search} onChange={setSearch} className="field">
            <Label>{t('Find the passage')}</Label>
            <Input autoComplete="off" data-testid="link-search" />
            <Text slot="description">{t('Type words of the passage, its code, or its document.')}</Text>
          </TextField>
          {all === null ? <p className="muted">{t('Looking for passages…')}</p> : null}
          {cut ? <p className="muted" data-testid="link-cut">{t('Only the first 5,000 coded passages are listed here, so a passage after them cannot be chosen from this list.')}</p> : null}
          {all !== null && candidates.length === 0 ? <p className="muted" data-testid="link-none">{t('No coded passage matches.')}</p> : null}
          <RadioGroup value={chosen === null ? '' : String(chosen)} onChange={(v) => setChosen(Number(v))} aria-label={t('Passages to link to')} className="link-choices" data-testid="link-choices">
            {candidates.map((s) => (
              <Radio key={s.coding_id} value={String(s.coding_id)} className="react-aria-Radio ac-radio link-choice" data-testid="link-choice">
                <span className="ac-radio-label">{s.kind === 'text' ? `“${s.text.replace(/\s+/g, ' ').slice(0, 140)}”` : s.text}</span>
                <span className="ac-radio-help">{`${s.source_name} · ${s.code_name}`}</span>
              </Radio>
            ))}
          </RadioGroup>
          <div className="actions">
            <Button type="button" onPress={close}>{t('Cancel')}</Button>
            <Button type="button" className="primary" onPress={() => void save()} isDisabled={busy} data-testid="link-save">{busy ? t('Linking…') : t('Link them')}</Button>
          </div>
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
