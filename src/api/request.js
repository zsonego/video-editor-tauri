import axios from 'axios';
import { showSessionExpiredDialog } from '../utils/sessionExpired';

const DEFAULT_BASE_URL = import.meta.env.VITE_API_BASE_URL || '';
const SESSION_EXPIRED_MESSAGE = '当前登录过期，请重新登录';

const service = axios.create({
  baseURL: DEFAULT_BASE_URL,
  timeout: 15000,
  headers: {
    'Content-Type': 'application/json',
  },
});

function getStoredToken() {
  return localStorage.getItem('token') || '';
}

function hasAuthorizationHeader(headers) {
  return Object.keys(headers).some(
    (key) => key.toLowerCase() === 'authorization',
  );
}

function withAuthHeader(headers = {}) {
  const requestHeaders = { ...headers };
  const token = getStoredToken();

  if (token && !hasAuthorizationHeader(requestHeaders)) {
    requestHeaders.Authorization = `Bearer ${token}`;
  }

  return requestHeaders;
}

function isLoginRequest(path) {
  return String(path || '').split('?')[0] === '/login';
}

function isTokenInvalidResponse(data, status) {
  const statusCode = Number(status);

  if (statusCode === 401) {
    return true;
  }

  if (!data || typeof data !== 'object') {
    return false;
  }

  const code = Number(data.code);
  const error = String(data.error || '').toLowerCase();
  const message = String(data.msg || data.message || '');

  return (
    code === 401 ||
    error.includes('cookie lose efficacy') ||
    (message.includes('认证失败') &&
      (message.toLowerCase().includes('token') || message.includes('RSA')))
  );
}

function handleTokenInvalid(path, data, status) {
  if (isLoginRequest(path) || !isTokenInvalidResponse(data, status)) {
    return false;
  }

  if (typeof window !== 'undefined') {
    showSessionExpiredDialog();
  }

  return true;
}

export async function request(path, options = {}) {
  const {
    method = 'POST',
    data,
    headers = {},
    signal,
    responseType = 'json',
  } = options;
  const normalizedMethod = method.toUpperCase();

  try {
    const response = await service.request({
      url: path,
      method: normalizedMethod,
      data: normalizedMethod === 'GET' ? undefined : (data ?? {}),
      params: normalizedMethod === 'GET' ? data : undefined,
      headers: withAuthHeader(headers),
      signal,
      responseType,
    });

    if (handleTokenInvalid(path, response.data, response.status)) {
      throw new Error(SESSION_EXPIRED_MESSAGE);
    }

    return response.data;
  } catch (error) {
    const responseData = error.response?.data;

    if (handleTokenInvalid(path, responseData, error.response?.status)) {
      throw new Error(SESSION_EXPIRED_MESSAGE);
    }

    const message =
      responseData?.msg ||
      responseData?.message ||
      (typeof responseData === 'string' ? responseData : '') ||
      error.message ||
      'Request failed';

    throw new Error(message);
  }
}
