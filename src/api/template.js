import { request } from "./request";

export function getTemplateCategories(data = {}) {
  return request("/api/template/categories", { data });
}

export function getTemplates(data = {}) {
  return request("/api/templates", { data });
}

export function getTemplateDetail(data = {}) {
  return request("/api/template/detail", { data });
}
