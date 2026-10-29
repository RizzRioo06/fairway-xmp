import { useEffect, useRef, useState } from 'react';
import Dialog from '../data/Dialog';
import { invoke } from '../bridge';
import * as api from './api';
import { complete, creationError, nameError, needsReview, planExpired } from './model';
import type { CreationJobStatus, CreationJobsSnapshot, NodeCreationPlan, TemplateCatalogSnapshot } from './types';
import TemplateSelection from './TemplateSelection';
import ProjectConfiguration, { type CreationDraft } from './ProjectConfiguration';
import CreationReview from './CreationReview';
import SetupProgress from './SetupProgress';
import CompletionRecovery from './CompletionRecovery';
const steps = ['Choose template', 'Configure project', 'Review setup', 'Create and install', 'Complete'];
export default function CreationWizard({ open, initialJobId, job, returnFocus, actionError, actionPending, onClose, onSelectJob, onRefresh, onCancel, onRecover, onNew, onView }: {
  open: boolean; initialJobId: string | null; job?: CreationJobStatus; returnFocus: HTMLElement | null;
  actionError: string | null; actionPending: boolean; onClose: () => void; onSelectJob: (id: string | null) => void;
  onRefresh: () => Promise<CreationJobsSnapshot | null>; onCancel: () => void; onRecover: () => void; onNew: () => void; onView: () => void;
}) {
  const [step, setStep] = useState(initialJobId ? 3 : 0);
  const [catalog, setCatalog] = useState<TemplateCatalogSnapshot | null>(null);
  const [draft, setDraft] = useState<CreationDraft>({ name: '', parent: '', runtime: '', template: '' });
  const [plan, setPlan] = useState<NodeCreationPlan | null>(null);
  const [confirmed, setConfirmed] = useState(false), [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null), [now, setNow] = useState(Date.now());
  const flight = useRef(false), mounted = useRef(true), heading = useRef<HTMLHeadingElement>(null);
  const submitted = useRef(initialJobId);
  const template = catalog?.catalog.templates.find(template => template.id === draft.template);
  const expired = !!plan && planExpired(plan, now);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; /* Native work outlives the view. */ }; }, []);
  useEffect(() => { if (!open) return; const timer = window.setInterval(() => setNow(Date.now()), 1000); return () => window.clearInterval(timer); }, [open]);
  useEffect(() => { if (open) heading.current?.focus({ preventScroll: true }); }, [step, open]);
  async function catalogLoad() {
    if (flight.current) return;
    flight.current = true; setPending(true); setError(null);
    try {
      const value = await api.nodeProjectTemplates();
      if (!mounted.current) return;
      setCatalog(value); setPlan(null); setConfirmed(false);
      setDraft(previous => ({ ...previous, template: value.catalog.templates.some(item => item.id === previous.template) ? previous.template : '', runtime: value.runtimes.some(item => item.id === previous.runtime) ? previous.runtime : '' }));
    } catch (error) { if (mounted.current) setError(creationError(error)); }
    finally { flight.current = false; if (mounted.current) setPending(false); }
  }
  useEffect(() => { if (open && !initialJobId && !catalog) void catalogLoad(); }, [open]);
  function change(value: CreationDraft) { setDraft(value); setPlan(null); setConfirmed(false); setError(null); }
  async function folder() {
    if (flight.current) return; flight.current = true; setPending(true); setError(null);
    try { const parent = await invoke<string | null>('pick_folder'); if (mounted.current && parent) change({ ...draft, parent }); }
    catch (error) { if (mounted.current) setError(creationError(error)); }
    finally { flight.current = false; if (mounted.current) setPending(false); }
  }
  async function makePlan() {
    if (!catalog) throw new Error('Load the template catalog first.');
    return api.planNodeProjectCreation({ requestId: crypto.randomUUID().replaceAll('-', ''), catalogRevision: catalog.catalogRevision, templateId: draft.template, projectName: draft.name, parent: draft.parent, runtimeId: draft.runtime, packageManager: 'npm' });
  }
  async function check(review = false) {
    if (flight.current) return; flight.current = true; setPending(true); setConfirmed(false); setError(null);
    try { const plan = await makePlan(); if (mounted.current) { setPlan(plan); setNow(Date.now()); if (review) setStep(2); } }
    catch (error) { if (mounted.current) { setPlan(null); setError(creationError(error)); } }
    finally { flight.current = false; if (mounted.current) setPending(false); }
  }
  async function create() {
    if (flight.current || !plan || !confirmed || planExpired(plan, Date.now())) return;
    flight.current = true; setPending(true); setError(null);
    let requested = false;
    try {
      await api.validateNodeCreationPlan({ planId: plan.id, digest: plan.digest });
      requested = true; submitted.current = plan.id; onSelectJob(plan.id); setStep(3);
      // This native operation generates in private staging, installs, then
      // publishes/registers. Calling Generate first would publish too early.
      await api.startNodeProjectInstallation({ planId: plan.id, digest: plan.digest, confirmed: true, installConfirmed: true });
      await onRefresh();
    } catch (error) {
      if (!mounted.current) return;
      const snapshot = requested ? await onRefresh() : null;
      if (requested && snapshot?.jobs.some(job => job.jobId === plan.id)) setError(creationError(error));
      else if (needsReview(error)) {
        submitted.current = null; onSelectJob(null); setConfirmed(false); setPlan(null); setStep(2);
        try { const fresh = await makePlan(); if (mounted.current) { setPlan(fresh); setNow(Date.now()); setError('The previous review expired or changed. A new native review is ready. Read it and confirm again.'); } }
        catch (refreshError) { if (mounted.current) { setStep(1); setError(creationError(refreshError)); } }
      } else setError(`${creationError(error)}${requested ? ' Check native setup status before sending another request.' : ''}`);
    } finally { flight.current = false; if (mounted.current) { setPending(false); setConfirmed(false); } }
  }
  if (!open) return null;
  const currentStep = job && complete(job) ? 4 : step;
  const canCheck = !!template && !!draft.parent && !nameError(draft.name) && !!catalog?.runtimes.some(runtime => runtime.id === draft.runtime);
  return <Dialog title="Create Node project" busy={false} onClose={onClose} onSubmit={() => {}} showSubmit={false} cancel={step >= 3 ? 'Hide setup' : 'Close'} returnFocus={returnFocus}>
    <div className="creation-wizard">
      <ol className="creation-steps" aria-label="Creation steps">{steps.map((label, index) => <li key={label} aria-current={currentStep === index ? 'step' : undefined}><span>{index + 1}</span>{label}</li>)}</ol>
      <h3 ref={heading} tabIndex={-1}>{steps[currentStep]}</h3>
      {(error || actionError) && <div className="notice error" role="alert">{error ?? actionError}</div>}
      {currentStep < 3 && !catalog && <><p role="status">{pending ? 'Loading templates and installed runtimes…' : 'The catalog could not be loaded.'}</p><button className="secondary" type="button" disabled={pending} onClick={() => { void catalogLoad(); }}>Retry catalog</button></>}
      {catalog && !catalog.supported && <p role="status">Project creation is available on macOS first. Your existing services remain available.</p>}
      {currentStep === 0 && catalog?.supported && <><TemplateSelection templates={catalog.catalog.templates} selected={draft.template} onSelect={id => change({ ...draft, template: id })} />{!catalog.catalog.templates.length && <p>No templates are available in this catalog.</p>}<div className="creation-actions"><button type="button" className="primary" disabled={!template || pending} onClick={() => setStep(1)}>Configure project</button></div></>}
      {currentStep === 1 && catalog && template && <><ProjectConfiguration draft={draft} catalog={catalog} template={template} plan={plan} pending={pending} onChange={change} onFolder={() => { void folder(); }} onRefresh={() => { void catalogLoad(); }} /><div className="creation-actions"><button type="button" className="secondary" disabled={pending} onClick={() => setStep(0)}>Back</button><button type="button" className="secondary" disabled={pending || !canCheck} onClick={() => { void check(); }}>{pending ? 'Checking…' : 'Check setup'}</button><button type="button" className="primary" disabled={pending || !plan} onClick={() => { setConfirmed(false); setNow(Date.now()); setStep(2); }}>Review setup</button></div></>}
      {currentStep === 2 && <>{plan ? <CreationReview plan={plan} expired={expired} confirmed={confirmed} onConfirm={setConfirmed} /> : <p>Refresh the native review before continuing.</p>}{expired && <p className="creation-error" role="alert">This review expired. Refresh it and confirm the new review.</p>}<div className="creation-actions"><button type="button" className="secondary" disabled={pending} onClick={() => { setConfirmed(false); setStep(1); }}>Back</button><button type="button" className="secondary" disabled={pending} onClick={() => { void check(true); }}>Refresh review</button><button type="button" className="primary" disabled={pending || !plan || expired || !confirmed} onClick={() => { void create(); }}>{pending ? 'Requesting setup…' : 'Create and install'}</button></div></>}
      {currentStep >= 3 && <><SetupProgress key={job?.installation?.runId ?? submitted.current ?? 'waiting'} job={job} pending={actionPending} onCancel={onCancel} onRefresh={() => { setError(null); void onRefresh(); }} />{job && <CompletionRecovery job={job} pending={actionPending} onRecover={onRecover} onNew={onNew} onView={onView} />}</>}
    </div>
  </Dialog>;
}
