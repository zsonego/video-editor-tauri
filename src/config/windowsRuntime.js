export const WINDOWS_RUNTIME_VERSION = String(
  import.meta.env.VITE_WINDOWS_RUNTIME_VERSION || '2026-09-09',
).trim();

export const WINDOWS_RUNTIME_DOWNLOAD_PATH = 'aicutx/dll/win.zip';

export function encodeBase64Url(value) {
  const bytes = new TextEncoder().encode(String(value || ''));
  let binary = '';
  bytes.forEach((byte) => {
    binary += String.fromCharCode(byte);
  });
  return btoa(binary)
    .replaceAll('+', '-')
    .replaceAll('/', '_')
    .replace(/=+$/g, '');
}
