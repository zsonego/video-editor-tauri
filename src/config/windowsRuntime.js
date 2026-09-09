export const WINDOWS_RUNTIME_VERSION = String(
  import.meta.env.VITE_WINDOWS_RUNTIME_VERSION || '2026-09-09',
).trim();

// 临时地址通过本地环境变量注入；后端接口就绪后改为接口返回地址和版本号。
export const WINDOWS_RUNTIME_DOWNLOAD_URL = String(
  import.meta.env.VITE_WINDOWS_RUNTIME_DOWNLOAD_URL || '',
).trim();
