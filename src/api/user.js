import { request } from "./request";

export function loginUser(data) {
  return request("/login", { data });
}

export function getUserInfo(data) {
  return request("/getInfo", { method: "GET", data });
}

export function logoutUser(data) {
  return request("/logout", { data });
}

export function resetPassword(data) {
  return request("/resetPwd/reset", { data });
}
