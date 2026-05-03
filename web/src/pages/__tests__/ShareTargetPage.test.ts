import { describe, it, expect } from 'vitest';
import { extractUrl } from '../ShareTargetPage';

describe('extractUrl', () => {
  it('returns url param when valid', () => {
    expect(extractUrl('https://example.com/article', null)).toBe('https://example.com/article');
  });

  it('extracts URL from text param when url is null', () => {
    expect(extractUrl(null, 'Check this out https://example.com/article great read')).toBe('https://example.com/article');
  });

  it('extracts URL from text param when url is empty', () => {
    expect(extractUrl('', 'https://example.com/article')).toBe('https://example.com/article');
  });

  it('returns null when no valid URL in either param', () => {
    expect(extractUrl(null, 'just some text')).toBeNull();
  });

  it('returns null when both params are null', () => {
    expect(extractUrl(null, null)).toBeNull();
  });

  it('rejects non-http protocols', () => {
    expect(extractUrl('ftp://example.com/file', null)).toBeNull();
  });

  it('accepts http protocol', () => {
    expect(extractUrl('http://example.com/article', null)).toBe('http://example.com/article');
  });

  it('extracts first URL from text with multiple URLs', () => {
    const text = 'https://first.com/a and https://second.com/b';
    expect(extractUrl(null, text)).toBe('https://first.com/a');
  });

  it('handles URL with query params', () => {
    expect(extractUrl('https://example.com/article?id=123&ref=share', null))
      .toBe('https://example.com/article?id=123&ref=share');
  });
});