import { request } from './request';

export function getUploadBuckets() {
  return request('/aicut/file/upload-buckets', { method: 'GET' });
}

export function getFileDownloadUrl(path) {
  return request('/aicut/file/download', {
    method: 'GET',
    data: { path },
  });
}

export function getBosDownloadUrl(path) {
  return request('/aicut/file/bos/download-url', {
    method: 'GET',
    data: { path },
  });
}
