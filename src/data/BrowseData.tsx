import { Activity, useState } from 'react';
import DataBrowser from './DataBrowser';
import PostgresPage from '../postgresql/PostgresPage';
import { MaximizeWorkspace } from '../Workspace';
export type SqlSourceRequest = { driver: 'mysql' | 'sqlite'; key: number };
/** One PostgreSQL controller survives source changes; SQL browsers retain Activity cleanup. */
export default function BrowseData({ active, ready, busy, postgresBusy, platform, start, open }: {
  active: boolean; ready: boolean; busy: boolean; postgresBusy: boolean; platform: string;
  start: () => void; open: (target: string) => void;
}) {
  const [postgres, setPostgres] = useState(false);
  const [opened, setOpened] = useState(false);
  const [sourceRequest, setSourceRequest] = useState<SqlSourceRequest>();
  function showPostgres() { setOpened(true); setPostgres(true); }
  function showSql(driver: SqlSourceRequest['driver']) {
    setSourceRequest(previous => ({ driver, key: (previous?.key ?? 0) + 1 })); setPostgres(false);
  }
  return <>
    <div hidden={postgres}><Activity mode={active && !postgres ? 'visible' : 'hidden'}><DataBrowser ready={ready} busy={busy} start={start} open={open} onPostgres={showPostgres} sourceRequest={sourceRequest} /></Activity></div>
    {opened && <div hidden={!postgres}>
      <div className="page-heading"><div><h1>Data browser</h1><p>Browse MySQL, SQLite and PostgreSQL databases.</p></div><MaximizeWorkspace id="postgres" title="PostgreSQL" /></div>
      <div className="db-source-tabs db-integrated-tabs" role="tablist" aria-label="Data source">
        <button role="tab" aria-selected={false} disabled={busy || postgresBusy} onClick={() => showSql('mysql')}>MySQL</button>
        <button role="tab" aria-selected={false} disabled={busy || postgresBusy} onClick={() => showSql('sqlite')}>SQLite</button>
        <button role="tab" aria-selected={true}>PostgreSQL</button>
      </div>
      <PostgresPage embedded active={active && postgres} busy={postgresBusy} platform={platform} />
    </div>}
  </>;
}
