import { invoke } from '@tauri-apps/api/core';
import { getBeautyAgeList } from '../api/beauty';

const BEAUTY_AGE_LIST_PATH = '/aicut/beauty-age/list';

async function writeBeautyAgeRequestLog({
  success,
  code = '',
  rowsCount = 0,
  message = '',
}) {
  try {
    await invoke('log_beauty_age_request', {
      success,
      code: String(code ?? ''),
      rowsCount: Number(rowsCount) || 0,
      message: String(message || ''),
    });
  } catch (error) {
    console.warn('[beauty] failed to write request log:', error);
  }
}

export async function refreshComposerBeautyPresets() {
  let response;
  try {
    response = await getBeautyAgeList();
  } catch (error) {
    await writeBeautyAgeRequestLog({
      success: false,
      code: error?.code,
      message: error?.message || String(error || '请求失败'),
    });
    throw error;
  }

  if (response?.code !== undefined && Number(response.code) !== 0) {
    await writeBeautyAgeRequestLog({
      success: false,
      code: response.code,
      message: response?.msg || '请求失败',
    });
    throw new Error(response?.msg || '获取一键美颜参数失败');
  }

  const rows = response?.rows;
  await writeBeautyAgeRequestLog({
    success: true,
    code: response?.code,
    rowsCount: rows?.length,
    message: BEAUTY_AGE_LIST_PATH,
  });
  if (!rows?.length) return false;

  const configJson = JSON.stringify({ beauty_presets: rows });
  await invoke('set_composer_config', { configJson });
  return true;
}
