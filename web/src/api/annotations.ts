import api from './client';

export interface Annotation {
  id: string;
  entry_id: string;
  quote: string;
  text: string;
  ranges: unknown[];
  is_orphaned: boolean;
  created_at: string;
  updated_at: string;
}

export async function listAnnotations(entryId: string): Promise<Annotation[]> {
  const res = await api.get(`/entries/${entryId}/annotations`);
  return res.data;
}

export async function createAnnotation(
  entryId: string,
  quote: string,
  text: string,
  ranges: unknown[] = []
): Promise<Annotation> {
  const res = await api.post(`/entries/${entryId}/annotations`, { quote, text, ranges });
  return res.data;
}

export async function updateAnnotation(id: string, text: string): Promise<Annotation> {
  const res = await api.patch(`/annotations/${id}`, { text });
  return res.data;
}

export async function deleteAnnotation(id: string): Promise<void> {
  await api.delete(`/annotations/${id}`);
}
