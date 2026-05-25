import { request } from "./request";

export function loginUser(data) {
  return request("/api/user/login", { data });
}

export function getUserInfo(data) {
  return request("/api/user/info", { data });
}
