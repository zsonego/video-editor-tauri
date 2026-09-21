import { computed, ref } from 'vue';

export const TEMPLATE_FACTORY_PERMISSION = 'aicut:template:factory';

const currentPermissions = ref([]);

function normalizePermissions(permissions) {
  if (!Array.isArray(permissions)) return [];
  return [...new Set(permissions.map((item) => String(item).trim()).filter(Boolean))];
}

function persistPermissions(permissions) {
  try {
    const stored = JSON.parse(localStorage.getItem('userInfo') || 'null') || {};
    localStorage.setItem(
      'userInfo',
      JSON.stringify({ ...stored, permissions }),
    );
  } catch {
    // 用户信息损坏时不覆盖原始存储，运行时权限仍然保持最新。
  }
}

export function setCurrentPermissions(permissions, options = {}) {
  const normalized = normalizePermissions(permissions);
  currentPermissions.value = normalized;
  if (options.persist !== false) persistPermissions(normalized);
}

export function clearCurrentPermissions() {
  currentPermissions.value = [];
}

export const canUseTemplateFactory = computed(() =>
  currentPermissions.value.includes(TEMPLATE_FACTORY_PERMISSION),
);

