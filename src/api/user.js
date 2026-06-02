import { request } from "./request";

export function loginUser(data) {
  return request("/login", { data });
}

export function getUserInfo(data) {
  return request("/getInfo", { method: "GET", data });
}
