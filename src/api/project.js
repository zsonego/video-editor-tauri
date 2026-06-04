import { request } from "./request";

export function createProject(data) {
  return request("/api/project/create", { data });
}

export function getMyProjects(data) {
  return request("/api/project/my", { data });
}
