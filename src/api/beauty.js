import { request } from './request';

export function getBeautyAgeList() {
  return request('/aicut/beauty-age/list', { method: 'GET' });
}
