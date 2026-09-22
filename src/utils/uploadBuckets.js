import { getUploadBuckets } from '../api/file';

export const UPLOAD_BUCKETS_STORAGE_KEY = 'uploadBuckets';

function uploadBucketsData(response) {
  if (response?.code !== undefined && Number(response.code) !== 0) {
    throw new Error(response?.msg || '获取上传桶配置失败');
  }

  const data = response?.data;
  if (!data || typeof data !== 'object' || Array.isArray(data)) {
    throw new Error('上传桶接口未返回有效配置');
  }

  const templateBucket = String(data['template-bucket'] || '').trim();
  if (!templateBucket) {
    throw new Error('上传桶配置缺少 template-bucket');
  }

  return { ...data };
}

export function setStoredUploadBuckets(data) {
  localStorage.setItem(UPLOAD_BUCKETS_STORAGE_KEY, JSON.stringify(data));
}

export function getStoredUploadBuckets() {
  try {
    const stored = JSON.parse(
      localStorage.getItem(UPLOAD_BUCKETS_STORAGE_KEY) || 'null',
    );
    return stored && typeof stored === 'object' && !Array.isArray(stored)
      ? stored
      : {};
  } catch {
    return {};
  }
}

export function getStoredUploadBucket(name) {
  return String(getStoredUploadBuckets()[name] || '').trim();
}

export function clearStoredUploadBuckets() {
  localStorage.removeItem(UPLOAD_BUCKETS_STORAGE_KEY);
}

export async function refreshStoredUploadBuckets() {
  const response = await getUploadBuckets();
  const data = uploadBucketsData(response);
  setStoredUploadBuckets(data);
  return data;
}
