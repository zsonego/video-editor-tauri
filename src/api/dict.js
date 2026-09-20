import { request } from './request';

export function getTransitionEffects() {
  return request('/system/dict/data/type/transition_effect', {
    method: 'GET',
  });
}
