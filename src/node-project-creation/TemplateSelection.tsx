import type { NodeTemplate } from './types';
export default function TemplateSelection({ templates, selected, onSelect }: { templates: NodeTemplate[]; selected: string; onSelect: (id: string) => void }) {
  return <fieldset className="creation-templates"><legend>Choose a starting point</legend>{templates.map(template => <label className={`creation-template${selected === template.id ? ' selected' : ''}`} key={template.id}>
    <input type="radio" name="creation-template" value={template.id} checked={selected === template.id} onChange={() => onSelect(template.id)} />
    <span><strong>{template.title}</strong><small>{template.language === 'typescript' ? 'TypeScript' : 'JavaScript'}</small><span>{template.description ?? template.provenance.note}</span></span>
  </label>)}</fieldset>;
}
