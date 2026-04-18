import { isAxiosError } from 'axios';

export function getErrorMessage(err: unknown, fallback: string): string {
  if (isAxiosError(err) && err.response?.data?.message) {
    return err.response.data.message;
  }
  return fallback;
}
