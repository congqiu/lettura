import { apiGet, apiPost, apiPatch, apiDel } from './client';
import type { components } from './schema';

export type Annotation = components['schemas']['Annotation'];

export async function listAnnotations(entryId: string): Promise<Annotation[]> {
  return apiGet<Annotation[]>(`/entries/${entryId}/annotations`);
}

export async function createAnnotation(
  entryId: string,
  quote: string,
  text: string,
  ranges: unknown[] = []
): Promise<Annotation> {
  return apiPost<Annotation>(`/entries/${entryId}/annotations`, { quote, text, ranges });
}

export async function updateAnnotation(id: string, text: string): Promise<Annotation> {
  return apiPatch<Annotation>(`/annotations/${id}`, { text });
}

export async function deleteAnnotation(id: string): Promise<void> {
  await apiDel(`/annotations/${id}`);
}