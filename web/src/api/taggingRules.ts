import { apiGet, apiPost, apiPatch, apiDel } from './client';
import type { components } from './schema';

export type TaggingRule = components['schemas']['TaggingRule'];
export type CreateTaggingRuleData = components['schemas']['CreateTaggingRule'];

export async function listRules(): Promise<TaggingRule[]> {
  return apiGet<TaggingRule[]>('/tagging-rules');
}

export async function createRule(data: CreateTaggingRuleData): Promise<TaggingRule> {
  return apiPost<TaggingRule>('/tagging-rules', data);
}

export async function updateRule(id: string, data: Partial<CreateTaggingRuleData>): Promise<TaggingRule> {
  return apiPatch<TaggingRule>(`/tagging-rules/${id}`, data);
}

export async function deleteRule(id: string): Promise<void> {
  await apiDel(`/tagging-rules/${id}`);
}