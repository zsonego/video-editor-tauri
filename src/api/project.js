import { request } from "./request";

export function createProject(data) {
  return request("/api/project/create", { data });
}

export function getProjectDetail(data) {
  return request("/api/project/detail", { data });
}

export function getMyProjects(data) {
  return request("/api/project/my", { data });
}

export function deleteProjects(data) {
  return request("/api/project/delete", { data });
}

export function renameProject(data) {
  return request("/api/project/rename", { data });
}

export function updateProject(data) {
  return request("/api/project/update", { data });
}

export function recordProjectExport(data) {
  return request("/api/project/export", { data });
}
