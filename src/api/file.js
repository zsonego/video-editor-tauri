import { request } from './request';

export function getFileDownloadUrl(path) {
  return request('/aicut/file/download', {
    method: 'GET',
    data: { path },
  });
}
