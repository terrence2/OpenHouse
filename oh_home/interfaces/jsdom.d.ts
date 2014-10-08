declare module "jsdom" {
    export function env(url: string, scripts: string[], cb:(err: Error, window:any) => void)
    export function serializeDocument(doc);
} 
