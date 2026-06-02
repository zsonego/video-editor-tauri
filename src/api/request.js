import axios from 'axios';

const DEFAULT_BASE_URL = import.meta.env.VITE_API_BASE_URL || '';

const service = axios.create({
  baseURL: DEFAULT_BASE_URL,
  timeout: 15000,
  headers: {
    'Content-Type': 'application/json',
  },
});

function readStorageJson(key) {
  try {
    return JSON.parse(localStorage.getItem(key) || 'null') || {};
  } catch {
    return {};
  }
}

function getStoredToken() {
  const userInfo = readStorageJson('userInfo');
  return userInfo?.token || '';
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

export async function request(path, options = {}) {
  const { method = 'POST', data, headers = {}, signal } = options;
  const normalizedMethod = method.toUpperCase();

  try {
    const response = await service.request({
      url: path,
      method: normalizedMethod,
      data: normalizedMethod === 'GET' ? undefined : (data ?? {}),
      params: normalizedMethod === 'GET' ? data : undefined,
      headers: withAuthHeader(headers),
      signal,
    });

    return response.data;
  } catch (error) {
    const responseData = error.response?.data;
    const message =
      responseData?.msg ||
      responseData?.message ||
      (typeof responseData === 'string' ? responseData : '') ||
      error.message ||
      'Request failed';

    throw new Error(message);
  }
}
