import { useEffect, useState } from 'react';
import { Button } from 'react-aria-components';
import { call } from '../bridge';
import { goTo } from '../state/actions';
import { useApp } from '../state/app';
import { Notice } from './common';
import { t } from '../i18n';

/**
 * Says so when the researcher's list of names is being applied to what leaves the project, on the screens where something leaves it
 * (the export window and the packet for a chatbot). Says nothing when it is off, so a screen without the list looks as it always did.
 */
export function NamesNotice(props: { what: 'file' | 'packet' }) {
  const version = useApp((s) => s.dataVersion);
  const [on, setOn] = useState(false);
  useEffect(() => {
    let live = true;
    call('deid.state', {})
      .then((s) => live && setOn(s.exports))
      .catch(() => live && setOn(false));
    return () => {
      live = false;
    };
  }, [version]);
  if (!on) return null;
  return (
    <Notice kind="info">
      {props.what === 'file' ? t('Your list of names is being applied: the names on it are replaced in this file. Pictures are left out.') : t('Your list of names is being applied: the names on it are replaced in this packet.')}{' '}
      <Button className="bk-link" onPress={() => goTo('privacy')}>{t('Open the list')}</Button>
    </Notice>
  );
}
