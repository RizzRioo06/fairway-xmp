import { useEffect, useState } from 'react';
import { complete, phaseLabel, projectName } from './model';
import { rememberCreation, rememberedCreation, useCreationJobs } from './useCreationJobs';
import CreationWizard from './CreationWizard';
import './creation.css';
export default function CreationWorkspace({ active, openRequest, returnFocus, onRegistered, onViewProject }: { active: boolean; openRequest: number; returnFocus: HTMLElement | null; onRegistered: () => Promise<void>; onViewProject: (id: string) => void }) {
  const controller = useCreationJobs(active, onRegistered);
  const [selected, setSelected] = useState<string | null>(rememberedCreation), [open, setOpen] = useState(false), [epoch, setEpoch] = useState(0);
  const job = controller.jobs.find(job => job.jobId === selected);
  function select(id: string | null) { setSelected(id); rememberCreation(id); }
  function newSetup() { select(null); setEpoch(value => value + 1); setOpen(true); }
  useEffect(() => { if (openRequest) newSetup(); }, [openRequest]);
  useEffect(() => { if (!active) setOpen(false); }, [active]);
  const important = controller.jobs.filter(job => job.active || job.recoveryRequired || job.jobId === selected);
  return <>
    {controller.error && <div className="notice error" role="alert">{controller.error} <button className="text-link" onClick={() => { void controller.refresh(); }}>Refresh setup history</button></div>}
    {controller.errors.length > 0 && <div className="notice error" role="alert">Some setup records could not be read. Files have been kept. {controller.errors.join(' ')}</div>}
    {(important.length > 0 || selected) && <section className="creation-history" aria-label="Node project setup jobs"><h2>Project setup</h2>{important.map(job => <div className="creation-history-row" key={job.jobId}><span><strong>{projectName(job)}</strong><small>{phaseLabel(job)}</small></span><button className="secondary" onClick={() => { select(job.jobId); setEpoch(value => value + 1); setOpen(true); }}>View setup for {projectName(job)}</button></div>)}{selected && !job && <p>Checking for the previously requested setup. No duplicate request will be sent. <button className="text-link" onClick={() => { void controller.refresh(); }}>Refresh setup status</button></p>}</section>}
    {controller.jobs.some(job => complete(job) && job.jobId !== selected) && <details className="creation-history"><summary>Completed setup history</summary>{controller.jobs.filter(job => complete(job) && job.jobId !== selected).map(job => <div className="creation-history-row" key={job.jobId}><span>{projectName(job)}</span><button className="secondary" onClick={() => { select(job.jobId); setEpoch(value => value + 1); setOpen(true); }}>View setup for {projectName(job)}</button></div>)}</details>}
    <CreationWizard key={epoch} open={open && active} initialJobId={selected} job={job} returnFocus={returnFocus} actionPending={controller.pending} actionError={controller.actionError?.jobId === selected ? controller.actionError.text : controller.error} onClose={() => setOpen(false)} onSelectJob={select} onRefresh={controller.refresh} onCancel={() => { if (job) void controller.perform(job, 'cancel'); }} onRecover={() => { if (job) void controller.perform(job, 'recover'); }} onNew={newSetup} onView={() => { setOpen(false); if (job?.projectId) onViewProject(job.projectId); }} />
  </>;
}
