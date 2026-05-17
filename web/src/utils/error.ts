import { isApiError } from '@/api/client';

export function getErrorMessage(err: unknown, fallback: string): string {
  if (isApiError(err)) {
    const body = err.body as { message?: string; error?: string } | undefined;
    if (body?.message) return body.message;
    if (body?.error) return body.error;
  }
  return fallback;
}