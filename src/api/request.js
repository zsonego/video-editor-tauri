import axios from 'axios';

const DEV_BASE_URL = '';
const PROD_BASE_URL = 'http://192.168.0.207:9527';

const DEFAULT_BASE_URL =
  import.meta.env.VITE_API_BASE_URL ||
  (import.meta.env.DEV ? DEV_BASE_URL : PROD_BASE_URL);

const service = axios.create({
  baseURL: DEFAULT_BASE_URL,
  timeout: 15000,
  headers: {
    'Content-Type': 'application/json',
  },
});

export async function request(path, options = {}) {
  const { method = 'POST', data, headers = {}, signal } = options;
  const normalizedMethod = method.toUpperCase();

  try {
    const response = await service.request({
      url: path,
      method: normalizedMethod,
      data: normalizedMethod === 'GET' ? undefined : (data ?? {}),
      params: normalizedMethod === 'GET' ? data : undefined,
      headers,
      signal,
    });

    return response.data;
  } catch (error) {
    const responseData = error.response?.data;
    const message =
      responseData?.message ||
      (typeof responseData === 'string' ? responseData : '') ||
      error.message ||
      'Request failed';

    throw new Error(message);
  }
}
