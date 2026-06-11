import { request } from "./request";

export function getTemplateCategories(data = {}) {
  return request("/api/template/category", { data });
}

export function getTemplates(data = {}) {
  return request("/api/template/list", { data });
}

export function getTemplateDetail(data = {}) {
  return request("/api/template/detail", { data });
}

export function favoriteTemplate(data = {}) {
  return request("/api/template/favorite", { data });
}

export function getFavoriteTemplates(data = {}) {
  return request("/api/template/favorite/list", { data });
}

export function downloadTemplateCover(templateId) {
  return request("/aicut/file/download", {
    method: "GET",
    data: {
      bucket: "template",
      path: `${templateId}/cover.png`,
    },
    responseType: "blob",
  });
}
