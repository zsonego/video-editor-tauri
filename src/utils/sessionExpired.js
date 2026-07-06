import { reactive } from 'vue';

export const sessionExpiredDialog = reactive({
  visible: false,
  message: '当前登录过期，是否重新登录？',
});

let activeResolve = null;
let activePromise = null;

export function showSessionExpiredDialog() {
  if (activePromise) {
    return activePromise;
  }

  sessionExpiredDialog.visible = true;
  sessionExpiredDialog.message = '当前登录过期，是否重新登录？';

  activePromise = new Promise((resolve) => {
    activeResolve = resolve;
  });

  return activePromise;
}

export function closeSessionExpiredDialog(confirmed) {
  sessionExpiredDialog.visible = false;

  if (activeResolve) {
    activeResolve(Boolean(confirmed));
  }

  activeResolve = null;
  activePromise = null;
}
