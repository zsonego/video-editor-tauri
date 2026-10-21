import { invoke } from '@tauri-apps/api/core';
import baiduSdkUrl from '../assets/vendor/baidubce-sdk.bundle.min.js?url';

const BOS_UPLOAD_CHUNK_SIZE = 20 * 1024 * 1024;
const BOS_UPLOAD_CONCURRENCY = 4;
const BOS_UPLOAD_MAX_RETRIES = 3;

let sdkLoadPromise;

async function loadBaiduBosSdk() {
  if (window.baidubce?.sdk?.BosClient) return window.baidubce.sdk;

  if (!sdkLoadPromise) {
    sdkLoadPromise = new Promise((resolve, reject) => {
      const script = document.createElement('script');
      script.src = baiduSdkUrl;
      script.onload = () => {
        const sdk = window.baidubce?.sdk;
        if (sdk?.BosClient) {
          resolve(sdk);
        } else {
          reject(new Error('百度 BCE SDK 已加载，但未找到 BosClient'));
        }
      };
      script.onerror = () => reject(new Error('百度 BCE SDK 加载失败'));
      document.head.appendChild(script);
    }).catch((error) => {
      sdkLoadPromise = null;
      throw error;
    });
  }

  return sdkLoadPromise;
}

async function retryBosUpload(action, partNumber) {
  let lastError;
  for (let attempt = 1; attempt <= BOS_UPLOAD_MAX_RETRIES; attempt += 1) {
    try {
      return await action();
    } catch (error) {
      lastError = error;
    }
  }
  throw new Error(
    `视频第 ${partNumber} 片上传失败：${lastError?.message || lastError}`,
  );
}

function readUploadId(response) {
  return String(response?.body?.uploadId || response?.uploadId || '').trim();
}

function readPartEtag(response) {
  return String(
    response?.http_headers?.etag ||
      response?.httpHeaders?.etag ||
      response?.headers?.etag ||
      response?.headers?.ETag ||
      '',
  ).trim();
}

export async function uploadLocalMp4ToBos({
  filePath,
  bucket,
  objectKey,
  endpoint,
  accessKeyId,
  secretAccessKey,
  sessionToken,
}) {
  if (
    !filePath ||
    !bucket ||
    !objectKey ||
    !endpoint ||
    !accessKeyId ||
    !secretAccessKey ||
    !sessionToken
  ) {
    throw new Error('百度 BOS 上传参数或临时凭证不完整');
  }

  const fileSize = Number(
    await invoke('get_bos_upload_file_size', { filePath }),
  );
  if (!Number.isSafeInteger(fileSize) || fileSize <= 0) {
    throw new Error('所选 MP4 文件为空或文件大小无效');
  }

  const totalParts = Math.ceil(fileSize / BOS_UPLOAD_CHUNK_SIZE);
  if (totalParts > 10000) {
    throw new Error('所选 MP4 文件过大，无法上传');
  }

  const sdk = await loadBaiduBosSdk();
  const client = new sdk.BosClient({
    endpoint,
    credentials: {
      ak: accessKeyId,
      sk: secretAccessKey,
    },
    sessionToken,
  });

  let uploadId = '';
  try {
    const initiated = await client.initiateMultipartUpload(bucket, objectKey);
    uploadId = readUploadId(initiated);
    if (!uploadId) {
      throw new Error('百度 BOS 未返回分片上传 ID');
    }

    const parts = new Array(totalParts);
    let nextPartIndex = 0;
    let stopped = false;
    const workers = Array.from(
      { length: Math.min(BOS_UPLOAD_CONCURRENCY, totalParts) },
      async () => {
        while (!stopped && nextPartIndex < totalParts) {
          const partIndex = nextPartIndex;
          nextPartIndex += 1;
          const partNumber = partIndex + 1;
          const expectedSize = Math.min(
            BOS_UPLOAD_CHUNK_SIZE,
            fileSize - partIndex * BOS_UPLOAD_CHUNK_SIZE,
          );
          try {
            const response = await retryBosUpload(async () => {
              const bytes = await invoke('read_bos_upload_file_chunk', {
                filePath,
                chunkIndex: partIndex,
                chunkSize: BOS_UPLOAD_CHUNK_SIZE,
              });
              const blob = new Blob([bytes], { type: 'video/mp4' });
              if (blob.size !== expectedSize) {
                throw new Error('读取到的视频分片大小不正确');
              }
              return client.uploadPartFromBlob(
                bucket,
                objectKey,
                uploadId,
                partNumber,
                blob.size,
                blob,
              );
            }, partNumber);
            const eTag = readPartEtag(response);
            if (!eTag) {
              throw new Error(`视频第 ${partNumber} 片上传成功，但未返回 ETag`);
            }
            parts[partIndex] = { partNumber, eTag };
          } catch (error) {
            stopped = true;
            throw error;
          }
        }
      },
    );

    const workerResults = await Promise.allSettled(workers);
    const failedWorker = workerResults.find(
      (result) => result.status === 'rejected',
    );
    if (failedWorker) throw failedWorker.reason;
    const completed = await client.completeMultipartUpload(
      bucket,
      objectKey,
      uploadId,
      parts,
    );

    return {
      bucket,
      objectKey,
      fileSize,
      uploadId,
      response: completed?.body || completed,
    };
  } catch (error) {
    if (uploadId) {
      try {
        await client.abortMultipartUpload(bucket, objectKey, uploadId);
      } catch (abortError) {
        console.warn('[hidden-media-asset] BOS multipart abort failed:', abortError);
      }
    }
    throw error;
  }
}
