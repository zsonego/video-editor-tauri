<script setup>
import { computed, onMounted, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { getUserInfo, loginUser } from '../api/user';
import { systemMessage } from '../utils/message';
import backgroundVideo from '../assets/background.mp4';
import boxImage from '../assets/box.png';
import logoImage from '../assets/logo1.png';
import userAgreementText from '../assets/user-agreement.txt?raw';

const emit = defineEmits(['login']);

const account = ref('');
const password = ref('');
const passwordVisible = ref(false);
const submitting = ref(false);
const agreementAccepted = ref(false);
const agreementVisible = ref(false);

const tenantDialogVisible = ref(false);
const tenantList = ref([]);
const selectedTenantId = ref('');
const terminalUuid = ref('');

const hasTenants = computed(() => tenantList.value.length > 0);

function normalizeRenterItem(item = {}) {
  const renterId = item.renterId || item.tenantId || '';
  const renterName = item.renterName || item.tenantName || '';

  return {
    ...item,
    renterId,
    renterName,
    tenantId: renterId,
    tenantName: renterName,
  };
}

function getLoginRenters(loginData) {
  const list = Array.isArray(loginData?.renterList)
    ? loginData.renterList
    : Array.isArray(loginData?.tenantList)
      ? loginData.tenantList
      : [];

  return list.map(normalizeRenterItem);
}

function getLoginIdentity(loginData, payload) {
  const tenants = getLoginRenters(loginData);
  const selectedRenterId = payload.renterId || payload.tenantId || '';
  const selectedTenant =
    tenants.find((tenant) => tenant.renterId === selectedRenterId) ||
    tenants.find((tenant) => tenant.passwordMatched) ||
    tenants[0] ||
    {};
  const renterId =
    selectedRenterId ||
    selectedTenant.renterId ||
    loginData?.renterId ||
    loginData?.tenantId ||
    '';

  return {
    tenantId: renterId,
    renterId,
    userId: selectedTenant.userId || loginData?.userId || '',
    tenantName:
      selectedTenant.renterName ||
      selectedTenant.tenantName ||
      loginData?.renterName ||
      loginData?.tenantName ||
      '',
    token: loginData?.token || '',
  };
}

function getLoginData(response) {
  return response?.data && typeof response.data === 'object'
    ? response.data
    : response || {};
}

function getLoginMessage(response) {
  return response?.msg || response?.message || response?.data?.msg || '';
}

function isSuccessResponse(response) {
  return response?.code === undefined || Number(response.code) === 0;
}

function buildLoginPayload(phone, loginPassword, terminalUuid, renterId = '') {
  return {
    phone,
    password: loginPassword,
    renterId,
    terminalUuid,
  };
}

function getUserInfoPayload(response) {
  const responseData = response?.data && typeof response.data === 'object'
    ? response.data
    : {};
  const user = response?.user || responseData.user || responseData || {};

  return {
    ...user,
    userId: user.userId || responseData.userId || '',
    tenantId: user.tenantId || user.renterId || responseData.tenantId || responseData.renterId || '',
    renterId: user.renterId || user.tenantId || responseData.renterId || responseData.tenantId || '',
    phone: user.phone || user.phonenumber || responseData.phone || '',
    roles: response?.roles || responseData.roles || [],
    permissions: response?.permissions || responseData.permissions || [],
  };
}

function saveLoginToken(token) {
  localStorage.setItem('token', token);
}

function saveUserInfo(userInfo, identity) {
  const storedUserInfo = {
    ...userInfo,
    userId: userInfo?.userId || identity?.userId || '',
    tenantId: userInfo?.tenantId || identity?.tenantId || '',
    renterId: userInfo?.renterId || identity?.renterId || '',
    tenantName: userInfo?.tenantName || identity?.tenantName || '',
    renterName: userInfo?.renterName || identity?.tenantName || '',
  };
  localStorage.setItem('userInfo', JSON.stringify(storedUserInfo));
}

function togglePasswordVisible() {
  passwordVisible.value = !passwordVisible.value;
}

function openAgreement() {
  agreementVisible.value = true;
}

function closeAgreement() {
  agreementVisible.value = false;
}

function handleBack() {
  if (window.history.length > 1) {
    window.history.back();
  }
}

function openTenantDialog(list) {
  tenantList.value = Array.isArray(list) ? list.map(normalizeRenterItem) : [];
  selectedTenantId.value = tenantList.value[0]?.tenantId || '';
  tenantDialogVisible.value = true;
}

function closeTenantDialog() {
  tenantDialogVisible.value = false;
}

async function getMachineCode() {
  try {
    return await invoke('get_machine_code');
  } catch {
    return '';
  }
}

async function ensureTerminalUuid() {
  const cachedTerminalUuid = localStorage.getItem('terminalUuid') || '';

  if (cachedTerminalUuid) {
    terminalUuid.value = cachedTerminalUuid;
    return cachedTerminalUuid;
  }

  const machineCode = await getMachineCode();
  if (machineCode) {
    localStorage.setItem('terminalUuid', machineCode);
    terminalUuid.value = machineCode;
  }

  return machineCode;
}

async function submitLogin(extra = {}) {
  const phone = account.value.trim();
  const loginPassword = password.value;

  if (!phone) {
    systemMessage.error('请输入账号');
    return;
  }

  if (!loginPassword) {
    systemMessage.error('请输入密码');
    return;
  }

  submitting.value = true;

  try {
    const currentTerminalUuid = terminalUuid.value || (await ensureTerminalUuid());
    const payload = buildLoginPayload(
      phone,
      loginPassword,
      currentTerminalUuid,
      extra.renterId || extra.tenantId || '',
    );

    const response = await loginUser(payload);
    const loginData = getLoginData(response);
    const result = Number(loginData?.result);
    const backendMessage = getLoginMessage(response);

    if (!isSuccessResponse(response)) {
      systemMessage.error(backendMessage || '鐧诲綍澶辫触');
      return;
    }

    if (result === 0) {
      const identity = getLoginIdentity(loginData, payload);
      const token = loginData?.token || '';

      if (!token) {
        systemMessage.error('登录成功，但未获取到 token');
        return;
      }

      saveLoginToken(token);
      const userInfoResponse = await getUserInfo();
      const userInfo = getUserInfoPayload(userInfoResponse);

      if (!userInfo.userId || !(userInfo.tenantId || userInfo.renterId)) {
        localStorage.removeItem('token');
        systemMessage.error('登录成功，但未获取到用户信息');
        return;
      }

      saveUserInfo(userInfo, identity);
      systemMessage.success(backendMessage || '登录成功');
      tenantDialogVisible.value = false;
      emit('login', {
        login: response,
        userInfo,
        identity,
      });
      return;
    }

    if (result === 1) {
      const list = getLoginRenters(loginData);
      if (!list.length) {
        systemMessage.error(
          backendMessage || '当前账号存在多个租户，请选择后重试',
        );
        return;
      }
      openTenantDialog(list);
      return;
    }

    if (result === 2) {
      systemMessage.error('用户已绑定 2 个电脑');
      return;
    }

    systemMessage.error(backendMessage || '登录失败');
  } catch (error) {
    localStorage.removeItem('token');
    systemMessage.error(error?.message || '登录请求失败');
  } finally {
    submitting.value = false;
  }
}

function handleLogin() {
  if (!agreementAccepted.value) {
    systemMessage.error('请先阅读并同意服务协议及隐私政策');
    return;
  }
  submitLogin();
}

function selectTenant(tenantId) {
  selectedTenantId.value = tenantId;
}

function confirmTenantSelection() {
  if (!selectedTenantId.value) {
    systemMessage.error('请选择一个租户');
    return;
  }

  tenantDialogVisible.value = false;
  submitLogin({ renterId: selectedTenantId.value });
}

onMounted(() => {
  document.title = '登录 - 鉴音';
  ensureTerminalUuid();
});
</script>

<template>
  <section class="login-page">
    <video
      class="login-background-video"
      :src="backgroundVideo"
      autoplay
      muted
      loop
      playsinline
    ></video>
    <div class="login-background-shade"></div>
    <main class="login-container">
      <div class="intro-panel">
        <img :src="logoImage" alt="鉴音" class="app-logo" />
        <div class="brand-copy">
          <h1>艾咔·专业版</h1>
          <p>释放无限创意，成就专业视界。专为高级视频创作者打造的精密工具。</p>
        </div>

        <div class="feature-list">
        <div class="feature-item">
          <div class="feature-icon">
            <span class="icon-text">4K</span>
          </div>
          <span>走向工业化标准的视频编辑器、零编辑经验也能秒出专业级大片</span>
        </div>

        <div class="feature-item">
          <div class="feature-icon">
            <svg class="feature-svg" viewBox="0 0 24 24" aria-hidden="true">
              <path d="M12 3 3 8l9 5 9-5-9-5Z" />
              <path d="m5 12 7 4 7-4" />
              <path d="m5 16 7 4 7-4" />
            </svg>
          </div>
          <span>模板化工作流 + 智能自动化</span>
        </div>

        <div class="feature-item">
          <div class="feature-icon">
            <svg class="feature-svg" viewBox="0 0 24 24" aria-hidden="true">
              <path
                d="M17.5 18H8a5 5 0 1 1 1-9.9 6 6 0 0 1 11.2 3A3.5 3.5 0 0 1 17.5 18Z"
              />
              <path d="m14 14 2 2 2-2" />
              <path d="M16 12v4" />
            </svg>
          </div>
          <span>专为“快剪场景”打造的傻瓜型视频编辑器</span>
        </div>
        </div>
      </div>

      <div class="login-card-wrap">
        <img :src="logoImage" alt="AICUT" class="login-brand-logo" />
        <div class="login-card" :style="{ backgroundImage: `url(${boxImage})` }">
          <button
            type="button"
            class="back-button"
            aria-label="返回"
            @click="handleBack"
          >
            <svg
              class="back-icon"
              viewBox="0 0 24 24"
              fill="none"
              aria-hidden="true"
            >
              <path
                d="M10 19l-7-7m0 0l7-7m-7 7h18"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
            </svg>
          </button>

          <div class="card-content">
            <div class="profile-icon">
              <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
                <path
                  d="M20 21a8 8 0 0 0-16 0"
                  stroke="currentColor"
                  stroke-width="2"
                  stroke-linecap="round"
                />
                <circle
                  cx="12"
                  cy="7"
                  r="4"
                  stroke="currentColor"
                  stroke-width="2"
                />
              </svg>
            </div>

            <h2>用户登录</h2>

            <form class="login-form" @submit.prevent="handleLogin">
              <div class="form-row">
                <label for="login-account">账号</label>
                <input
                  id="login-account"
                  v-model="account"
                  class="input-underline"
                  placeholder="输入您的用户名"
                  type="tel"
                  autocomplete="username"
                />
              </div>

              <div class="form-row password-row">
                <label for="login-password">密码</label>
                <input
                  id="login-password"
                  v-model="password"
                  class="input-underline password-input"
                  placeholder="输入登录密码"
                  :type="passwordVisible ? 'text' : 'password'"
                  autocomplete="current-password"
                />
                <button
                  type="button"
                  class="toggle-password"
                  :aria-label="passwordVisible ? '隐藏密码' : '显示密码'"
                  @click="togglePasswordVisible"
                >
                  <svg
                    v-if="passwordVisible"
                    viewBox="0 0 24 24"
                    fill="none"
                    aria-hidden="true"
                  >
                    <path
                      d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12Z"
                      stroke="currentColor"
                      stroke-width="2"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                    />
                    <circle
                      cx="12"
                      cy="12"
                      r="3"
                      stroke="currentColor"
                      stroke-width="2"
                    />
                  </svg>
                  <svg
                    v-else
                    viewBox="0 0 24 24"
                    fill="none"
                    aria-hidden="true"
                  >
                    <path
                      d="M3 3l18 18"
                      stroke="currentColor"
                      stroke-width="2"
                      stroke-linecap="round"
                    />
                    <path
                      d="M10.6 10.7A3 3 0 0 0 13.3 14"
                      stroke="currentColor"
                      stroke-width="2"
                      stroke-linecap="round"
                    />
                    <path
                      d="M9.9 5.2A10.6 10.6 0 0 1 12 5c6.5 0 10 7 10 7a18 18 0 0 1-3.1 4.1"
                      stroke="currentColor"
                      stroke-width="2"
                      stroke-linecap="round"
                    />
                    <path
                      d="M6.7 6.7C3.7 8.7 2 12 2 12s3.5 7 10 7a9.7 9.7 0 0 0 5.2-1.5"
                      stroke="currentColor"
                      stroke-width="2"
                      stroke-linecap="round"
                    />
                  </svg>
                </button>
              </div>

              <div class="login-links">
                <button type="button">忘记密码?</button>
              </div>

              <div class="agreement-consent">
                <input
                  id="agreement-consent"
                  v-model="agreementAccepted"
                  type="checkbox"
                />
                <span
                  class="agreement-check material-symbols-outlined"
                  aria-hidden="true"
                  @click="agreementAccepted = !agreementAccepted"
                  >{{
                    agreementAccepted ? 'check_box' : 'check_box_outline_blank'
                  }}</span
                >
                <label for="agreement-consent">我已阅读并同意</label>
                <button type="button" @click="openAgreement">
                  《服务协议及隐私政策》
                </button>
              </div>

              <div class="submit-wrap">
                <button
                  class="login-button"
                  type="submit"
                  :disabled="submitting || !agreementAccepted"
                >
                  {{ submitting ? '登录中...' : '登录' }}
                </button>
              </div>
            </form>
          </div>
        </div>
      </div>
    </main>

    <transition name="tenant-fade">
      <div
        v-if="agreementVisible"
        class="agreement-mask"
        @click.self="closeAgreement"
      >
        <section class="agreement-dialog" aria-modal="true" role="dialog">
          <header class="agreement-dialog-header">
            <div>
              <h3>服务协议及隐私政策</h3>
              <p>请仔细阅读以下全部内容</p>
            </div>
            <button
              type="button"
              class="dialog-close"
              aria-label="关闭"
              @click="closeAgreement"
            >
              <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
                <path
                  d="M6 6l12 12M18 6 6 18"
                  stroke="currentColor"
                  stroke-width="2"
                  stroke-linecap="round"
                />
              </svg>
            </button>
          </header>
          <div class="agreement-document">{{ userAgreementText }}</div>
          <footer class="agreement-dialog-footer">
            <button type="button" @click="closeAgreement">关闭</button>
          </footer>
        </section>
      </div>
    </transition>

    <transition name="tenant-fade">
      <div
        v-if="tenantDialogVisible"
        class="tenant-mask"
        @click.self="closeTenantDialog"
      >
        <div class="tenant-dialog">
          <div class="tenant-dialog-header">
            <div>
              <h3>选择租户</h3>
              <p>该账号关联了多个租户，请选择后继续登录。</p>
            </div>
            <button
              type="button"
              class="dialog-close"
              aria-label="关闭"
              @click="closeTenantDialog"
            >
              <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
                <path
                  d="M6 6l12 12M18 6 6 18"
                  stroke="currentColor"
                  stroke-width="2"
                  stroke-linecap="round"
                />
              </svg>
            </button>
          </div>

          <div class="tenant-list">
            <button
              v-for="tenant in tenantList"
              :key="tenant.tenantId"
              type="button"
              class="tenant-item"
              :class="{ selected: selectedTenantId === tenant.tenantId }"
              @click="selectTenant(tenant.tenantId)"
            >
              <div class="tenant-main">
                <div class="tenant-name">
                  {{ tenant.tenantName || '租户' }}
                </div>
              </div>
              <div class="tenant-meta">
                <span
                  class="tenant-radio"
                  :class="{ checked: selectedTenantId === tenant.tenantId }"
                ></span>
              </div>
            </button>
          </div>

          <div class="tenant-actions">
            <button
              type="button"
              class="tenant-secondary"
              @click="closeTenantDialog"
            >
              取消
            </button>
            <button
              type="button"
              class="tenant-primary"
              :disabled="!hasTenants"
              @click="confirmTenantSelection"
            >
              确认登录
            </button>
          </div>
        </div>
      </div>
    </transition>
  </section>
</template>

<style scoped>
.login-page {
  width: 100%;
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  padding: 16px;
  background-size: cover;
  background-position: center;
  background-repeat: no-repeat;
  font-family:
    Inter,
    ui-sans-serif,
    system-ui,
    -apple-system,
    BlinkMacSystemFont,
    'Segoe UI',
    sans-serif;
}

.login-container {
  width: 100%;
  max-width: 1240px;
  height: calc(100vh - 32px);
  max-height: 820px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 32px;
}

.intro-panel {
  width: 50%;
  display: flex;
  flex-direction: column;
  gap: 28px;
  padding: 40px;
  color: #fff;
}

.app-logo {
  width: min(320px, 80vw);
  height: auto;
  object-fit: contain;
}

.brand-copy {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.brand-copy h1 {
  margin: 0;
  color: #fff;
  font-size: 56px;
  line-height: 1;
  font-weight: 700;
  letter-spacing: 0;
}

.brand-copy p {
  max-width: 440px;
  margin: 0;
  color: rgba(255, 255, 255, 0.82);
  font-size: 18px;
  line-height: 1.7;
}

.feature-list {
  display: flex;
  flex-direction: column;
  gap: 24px;
  padding-top: 20px;
}

.feature-item {
  display: flex;
  align-items: center;
  gap: 16px;
  color: rgba(255, 255, 255, 0.9);
  font-size: 18px;
  line-height: 1.5;
  font-weight: 500;
}

.feature-icon {
  width: 40px;
  height: 40px;
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.1);
  transition: background 0.2s ease;
}

.feature-item:hover .feature-icon {
  background: rgba(255, 255, 255, 0.2);
}

.icon-text {
  font-size: 20px;
  line-height: 1;
  font-weight: 700;
}

.feature-svg {
  width: 20px;
  height: 20px;
  fill: none;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 2;
}

.login-card-wrap {
  width: 440px;
  flex: 0 0 auto;
}

.login-card {
  position: relative;
  padding: 48px;
  border: 1px solid rgba(255, 255, 255, 0.14);
  border-radius: 32px;
  background: rgba(255, 255, 255, 0.1);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.35);
  backdrop-filter: blur(24px);
  -webkit-backdrop-filter: blur(24px);
}

.back-button {
  position: absolute;
  top: 24px;
  left: 24px;
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  background: transparent;
  color: rgba(255, 255, 255, 0.7);
  cursor: pointer;
}

.back-icon {
  width: 24px;
  height: 24px;
}

.card-content {
  display: flex;
  flex-direction: column;
  align-items: center;
}

.profile-icon {
  width: 80px;
  height: 80px;
  display: flex;
  align-items: center;
  justify-content: center;
  margin-bottom: 28px;
  border: 1px solid rgba(255, 255, 255, 0.18);
  border-radius: 999px;
  background: rgba(26, 35, 126, 0.55);
}

.profile-icon svg {
  width: 40px;
  height: 40px;
  color: rgba(255, 255, 255, 0.92);
}

.card-content h2 {
  margin: 0 0 36px;
  color: rgba(255, 255, 255, 0.74);
  font-size: 12px;
  line-height: 1.4;
  font-weight: 600;
  letter-spacing: 0.2em;
  text-transform: uppercase;
}

.login-form {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 28px;
}

.form-row {
  position: relative;
  display: flex;
  align-items: flex-end;
  gap: 12px;
}

.form-row label {
  flex: 0 0 auto;
  padding-bottom: 8px;
  color: rgba(255, 255, 255, 0.8);
  font-size: 14px;
  line-height: 1.4;
  font-weight: 600;
}

.input-underline {
  width: 100%;
  min-width: 0;
  padding: 0 0 8px;
  border: 0;
  border-bottom: 1px solid rgba(255, 255, 255, 0.28);
  border-radius: 0;
  background: transparent;
  color: #fff;
  font: inherit;
  font-size: 14px;
  line-height: 1.4;
  outline: none;
  box-shadow: none;
}

.input-underline::placeholder {
  color: rgba(255, 255, 255, 0.42);
}

.input-underline:focus {
  border-bottom-color: #fff;
}

.password-input {
  padding-right: 40px;
}

.password-input::-ms-reveal,
.password-input::-ms-clear {
  display: none;
  width: 0;
  height: 0;
}

.toggle-password {
  position: absolute;
  right: 0;
  bottom: 8px;
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  background: transparent;
  color: rgba(255, 255, 255, 0.42);
  cursor: pointer;
}

.toggle-password svg {
  width: 20px;
  height: 20px;
}

.submit-wrap {
  padding-top: 18px;
}

.login-button,
.tenant-primary,
.tenant-secondary {
  border: 0;
  font: inherit;
  cursor: pointer;
}

.login-button {
  width: 100%;
  padding: 15px 16px;
  border-radius: 14px;
  background: #152b94;
  color: #fff;
  font-weight: 600;
  box-shadow: 0 0 20px rgba(21, 43, 148, 0.35);
  transition:
    transform 0.2s ease,
    background 0.2s ease;
}

.login-button:hover:not(:disabled) {
  background: #1e40af;
  transform: translateY(-1px);
}

.login-button:disabled {
  opacity: 0.7;
  cursor: not-allowed;
}

.tenant-mask {
  position: fixed;
  inset: 0;
  z-index: 60;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
  background: rgba(2, 6, 23, 0.62);
  backdrop-filter: blur(8px);
}

.tenant-dialog {
  width: min(520px, 100%);
  padding: 22px;
  border: 1px solid rgba(255, 255, 255, 0.14);
  border-radius: 24px;
  background: rgba(7, 18, 42, 0.96);
  box-shadow: 0 24px 80px rgba(0, 0, 0, 0.45);
}

.tenant-dialog-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 16px;
}

.tenant-dialog-header h3 {
  margin: 0;
  color: #fff;
  font-size: 18px;
  line-height: 1.3;
}

.tenant-dialog-header p {
  margin: 6px 0 0;
  color: rgba(217, 226, 255, 0.62);
  font-size: 13px;
  line-height: 1.5;
}

.dialog-close {
  width: 36px;
  height: 36px;
  display: grid;
  place-items: center;
  border: 0;
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.06);
  color: rgba(255, 255, 255, 0.72);
}

.dialog-close svg {
  width: 18px;
  height: 18px;
}

.tenant-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
  max-height: 320px;
  overflow: auto;
  padding-right: 2px;
}

.tenant-item {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
  padding: 14px 16px;
  border: 1px solid rgba(74, 142, 255, 0.16);
  border-radius: 16px;
  background: rgba(255, 255, 255, 0.04);
  color: #fff;
  text-align: left;
}

.tenant-item.selected {
  border-color: rgba(74, 142, 255, 0.7);
  background: rgba(74, 142, 255, 0.12);
}

.tenant-main {
  min-width: 0;
}

.tenant-name {
  font-size: 15px;
  font-weight: 600;
  line-height: 1.3;
}

.tenant-meta {
  display: flex;
  align-items: center;
  gap: 10px;
  flex: 0 0 auto;
}

.tenant-radio {
  width: 18px;
  height: 18px;
  border: 2px solid rgba(217, 226, 255, 0.36);
  border-radius: 999px;
}

.tenant-radio.checked {
  border-color: #4a8eff;
  box-shadow: inset 0 0 0 4px #4a8eff;
}

.tenant-actions {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
  margin-top: 18px;
}

.tenant-secondary {
  padding: 11px 16px;
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.06);
  color: rgba(255, 255, 255, 0.88);
}

.tenant-primary {
  padding: 11px 18px;
  border-radius: 12px;
  background: #4a8eff;
  color: #fff;
}

.tenant-primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.tenant-fade-enter-active,
.tenant-fade-leave-active {
  transition: opacity 0.18s ease;
}

.tenant-fade-enter-from,
.tenant-fade-leave-to {
  opacity: 0;
}

@media (max-width: 767px) {
  .login-page {
    align-items: flex-start;
    overflow: auto;
  }

  .login-container {
    height: auto;
    min-height: calc(100vh - 32px);
    flex-direction: column;
    justify-content: center;
  }

  .intro-panel {
    width: 100%;
    padding: 24px 20px 0;
  }

  .brand-copy h1 {
    font-size: 44px;
  }

  .brand-copy p {
    font-size: 16px;
  }

  .feature-list {
    gap: 18px;
    padding-top: 8px;
  }

  .feature-item {
    align-items: flex-start;
    font-size: 16px;
  }

  .login-card-wrap {
    width: 100%;
  }

  .login-card {
    padding: 40px 28px;
  }
}

.login-page {
  position: relative;
  justify-content: flex-end;
  padding: 0;
  background: #07121d;
}

.login-background-video,
.login-background-shade {
  position: fixed;
  inset: 0;
  width: 100%;
  height: 100%;
}

.login-background-video {
  z-index: 0;
  object-fit: cover;
}

.login-background-shade {
  z-index: 1;
  pointer-events: none;
  background:
    radial-gradient(circle at 75% 42%, rgba(23, 84, 128, 0.14), transparent 28%),
    linear-gradient(90deg, rgba(0, 0, 0, 0.08), rgba(2, 7, 15, 0.18));
}

.login-container {
  position: relative;
  z-index: 2;
  max-width: none;
  height: 100vh;
  max-height: none;
  justify-content: flex-end;
  padding-right: clamp(86px, 15vw, 220px);
  gap: 0;
}

.intro-panel {
  display: none;
}

.login-card-wrap {
  width: 320px;
  display: flex;
  flex-direction: column;
  align-items: center;
  margin-top: -56px;
}

.login-brand-logo {
  width: 148px;
  height: auto;
  margin-bottom: 8px;
  object-fit: contain;
}

.login-card {
  width: 320px;
  height: 376px;
  padding: 0;
  border: 0;
  border-radius: 0;
  background-color: transparent;
  background-repeat: no-repeat;
  background-position: center;
  background-size: 100% 100%;
  box-shadow: none;
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
}

.back-button,
.profile-icon {
  display: none;
}

.card-content {
  height: 100%;
  padding: 66px 34px 46px;
}

.card-content h2 {
  position: relative;
  margin: 0 0 38px;
  color: #23d9ff;
  font-size: 21px;
  line-height: 1;
  font-weight: 800;
  letter-spacing: 0.12em;
  text-align: center;
  text-transform: none;
}

.card-content h2::after {
  content: 'USER LOGIN';
  display: block;
  margin-top: 7px;
  color: rgba(35, 217, 255, 0.78);
  font-size: 9px;
  line-height: 1;
  font-weight: 700;
  letter-spacing: 0.22em;
}

.login-form {
  gap: 10px;
}

.form-row {
  height: 36px;
  align-items: center;
  gap: 0;
}

.form-row label {
  position: absolute;
  left: 16px;
  top: 50%;
  z-index: 1;
  width: 20px;
  height: 20px;
  padding: 0;
  transform: translateY(-50%);
  color: rgba(180, 207, 244, 0.72);
  font-size: 0;
}

.form-row label::before {
  content: 'person';
  font-family: 'Material Symbols Outlined';
  font-size: 20px;
  line-height: 1;
  font-weight: 300;
}

.password-row label::before {
  content: 'vpn_key';
}

.input-underline {
  height: 36px;
  padding: 0 44px 0 48px;
  border: 1px solid rgba(191, 218, 255, 0.68);
  border-radius: 999px;
  background: rgba(12, 25, 45, 0.18);
  color: rgba(230, 242, 255, 0.92);
  font-size: 13px;
  line-height: 36px;
  box-shadow:
    inset 0 0 12px rgba(77, 144, 214, 0.16),
    0 0 8px rgba(73, 143, 220, 0.12);
}

.input-underline::placeholder {
  color: rgba(202, 222, 255, 0.46);
}

.input-underline:focus {
  border-color: rgba(178, 213, 255, 0.96);
  box-shadow:
    inset 0 0 14px rgba(77, 144, 214, 0.2),
    0 0 12px rgba(73, 143, 220, 0.22);
}

.toggle-password {
  right: 14px;
  bottom: 50%;
  width: 22px;
  height: 22px;
  transform: translateY(50%);
  color: rgba(202, 222, 255, 0.74);
}

.toggle-password svg {
  width: 20px;
  height: 20px;
}

.login-links {
  display: flex;
  justify-content: flex-end;
  gap: 16px;
  padding-right: 6px;
  margin: 4px 0 8px;
}

.login-links button {
  padding: 0;
  border: 0;
  background: transparent;
  color: rgba(194, 216, 245, 0.68);
  font-size: 12px;
  line-height: 1;
  cursor: pointer;
}

.login-links button:hover {
  color: #23d9ff;
}

.agreement-consent {
  display: flex;
  align-items: center;
  gap: 5px;
  min-width: 0;
  margin: 0 3px 4px;
  color: rgba(194, 216, 245, 0.72);
  font-size: 10px;
  line-height: 1.35;
  white-space: nowrap;
}

.agreement-consent input {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  opacity: 0;
  pointer-events: none;
}

.agreement-check {
  flex: 0 0 auto;
  color: rgba(194, 216, 245, 0.62);
  font-size: 17px;
  line-height: 1;
  cursor: pointer;
}

.agreement-consent input:checked + .agreement-check {
  color: #23d9ff;
}

.agreement-consent label {
  cursor: pointer;
}

.agreement-consent button {
  min-width: 0;
  padding: 0;
  border: 0;
  background: transparent;
  color: #23d9ff;
  font: inherit;
  cursor: pointer;
}

.agreement-consent button:hover {
  color: #8cebff;
}

.submit-wrap {
  padding-top: 0;
}

.login-button {
  height: 39px;
  padding: 0 16px;
  border-radius: 4px;
  background: #a9c5f0;
  color: #fff;
  font-size: 17px;
  font-weight: 800;
  letter-spacing: 0.12em;
  box-shadow: 0 0 16px rgba(169, 197, 240, 0.3);
}

.login-button:hover:not(:disabled) {
  background: #bad3f8;
  transform: none;
}

.login-button:disabled {
  opacity: 0.72;
}

.agreement-mask {
  position: fixed;
  inset: 0;
  z-index: 80;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  background: rgba(2, 6, 23, 0.7);
  backdrop-filter: blur(10px);
}

.agreement-dialog {
  width: min(820px, 100%);
  max-height: min(82vh, 780px);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border: 1px solid rgba(130, 190, 255, 0.24);
  border-radius: 14px;
  background: rgba(7, 18, 42, 0.98);
  box-shadow: 0 28px 90px rgba(0, 0, 0, 0.55);
}

.agreement-dialog-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 18px;
  padding: 20px 24px 16px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}

.agreement-dialog-header h3 {
  margin: 0;
  color: #fff;
  font-size: 18px;
  line-height: 1.35;
}

.agreement-dialog-header p {
  margin: 5px 0 0;
  color: rgba(202, 222, 255, 0.58);
  font-size: 12px;
}

.agreement-document {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 22px 28px;
  background: rgba(0, 0, 0, 0.12);
  color: rgba(230, 240, 255, 0.82);
  font-size: 13px;
  line-height: 1.85;
  white-space: pre-wrap;
  word-break: break-word;
}

.agreement-document::-webkit-scrollbar {
  width: 6px;
}

.agreement-document::-webkit-scrollbar-thumb {
  border-radius: 999px;
  background: rgba(35, 217, 255, 0.3);
}

.agreement-dialog-footer {
  display: flex;
  justify-content: flex-end;
  padding: 14px 20px;
  border-top: 1px solid rgba(255, 255, 255, 0.08);
}

.agreement-dialog-footer button {
  min-width: 88px;
  height: 36px;
  border: 1px solid rgba(35, 217, 255, 0.34);
  border-radius: 8px;
  background: rgba(35, 217, 255, 0.1);
  color: #b9f4ff;
  font-size: 13px;
  font-weight: 700;
  cursor: pointer;
}

.agreement-dialog-footer button:hover {
  background: rgba(35, 217, 255, 0.18);
}

@media (max-width: 900px) {
  .login-container {
    justify-content: center;
    padding: 0 24px;
  }

  .login-card-wrap {
    margin-top: 0;
  }

  .agreement-mask {
    padding: 12px;
  }

  .agreement-dialog {
    max-height: 88vh;
  }

  .agreement-document {
    padding: 18px;
  }
}
</style>
