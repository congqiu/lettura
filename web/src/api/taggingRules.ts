import api from './client';

export interface TaggingRule {
  id: string;
  rule: {
    field: string;
    operator: string;
    value: string;
  };
  tags: string[];
  priority: number;
  created_at: string;
}

export interface CreateTaggingRuleData {
  rule: {
    field: string;
    operator: string;
    value: string;
  };
  tags: string[];
  priority?: number;
}

export async function listRules(): Promise<TaggingRule[]> {
  const res = await api.get('/tagging-rules');
  return res.data;
}

export async function createRule(data: CreateTaggingRuleData): Promise<TaggingRule> {
  const res = await api.post('/tagging-rules', data);
  return res.data;
}

export async function updateRule(id: string, data: Partial<CreateTaggingRuleData>): Promise<TaggingRule> {
  const res = await api.patch(`/tagging-rules/${id}`, data);
  return res.data;
}

export async function deleteRule(id: string): Promise<void> {
  await api.delete(`/tagging-rules/${id}`);
}
