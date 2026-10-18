import type { ActivityResult } from '#lib/generated/api.ts';
import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';

export const resultLabel = (result: ActivityResult): string => {
  switch (result) {
    case 'accepted':
      return i18n.t.activity.resultAccepted;
    case 'no_op':
      return i18n.t.activity.resultNoOp;
    case 'rejected':
      return i18n.t.activity.resultRejected;
  }
};

export const operationLabel = (kind: string): string => {
  return i18n.t.activity.ops[kind] ?? kind;
};

export const rejectionLabel = (code: string): string => {
  return i18n.t.activity.rejections[code] ?? code;
};

export function parseResult(value: string | null): ActivityResult | undefined {
  return value === 'accepted' || value === 'no_op' || value === 'rejected' ? value : undefined;
}
