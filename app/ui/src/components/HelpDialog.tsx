import { Button, Dialog, Heading, Modal, ModalOverlay } from 'react-aria-components';
import { SHORTCUT_SECTIONS } from '../lib/shortcuts';
import { appStore } from '../state/app';
import { t } from '../i18n';

export function HelpDialog() {
  const close = () => appStore.set({ dialog: null });
  return (
    <ModalOverlay isOpen isDismissable onOpenChange={(o) => !o && close()} className="overlay help-overlay">
      <Modal className="modal modal-wide help-modal">
        <Dialog className="dialog help-dialog" aria-labelledby="help-title">
          <Heading slot="title" id="help-title" className="dialog-title">{t('Keyboard shortcuts')}</Heading>
          <p className="help">{t('Everything in PrismQDA can be done from the keyboard. Print this page to keep it beside you.')}</p>
          {SHORTCUT_SECTIONS.map((section, i) => (
            <section key={section.title} aria-labelledby={`help-sec-${i}`} className="help-section">
              <h3 id={`help-sec-${i}`}>{section.title}</h3>
              {section.note ? <p className="help">{section.note}</p> : null}
              <table className="data shortcuts">
                <caption className="visually-hidden">{section.title}</caption>
                <thead>
                  <tr>
                    <th scope="col">{t('Keys')}</th>
                    <th scope="col">{t('What they do')}</th>
                  </tr>
                </thead>
                <tbody>
                  {section.entries.map((e) => (
                    <tr key={e.keys}>
                      <th scope="row">
                        <kbd>{e.keys}</kbd>
                      </th>
                      <td>{e.does}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </section>
          ))}
          <div className="actions no-print">
            <Button className="primary" onPress={() => window.print()}>{t('Print this list')}</Button>
            <Button onPress={close}>{t('Close')}</Button>
          </div>
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
