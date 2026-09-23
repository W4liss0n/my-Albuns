import { useMemo, useState } from 'react';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { expect, test, vi } from 'vitest';
import { ImageViewer } from './ImageViewer';
import { useImageViewerSession } from '../application/useImageViewerSession';
import type { ImageViewerWindowPort, ViewerPresentation, ViewerCorrectionAction } from '../application/imageViewerWindow';
import type { ProjectMutationRunner } from '../application/projectMutation';
import { representativeProjection } from '../test/projectFixtures';
import { acquireFaces, type Face } from '../image-viewer/faceLandmarks';
vi.mock('../image-viewer/faceLandmarks', async original => ({ ...(await original()), acquireFaces: vi.fn() }));
const detectFaces = vi.fn<(image: HTMLImageElement) => Promise<Face[]>>();

test.each(['reference', 'target'] as const)('a refreshed %s requires new landmarks and ignores the earlier preparation', async (side) => {
  const oldFace = [{ x: .4, y: .3, z: 0 }, { x: .6, y: .5, z: 0 }];
  detectFaces.mockReset();
  detectFaces.mockResolvedValue([oldFace]);
  vi.mocked(acquireFaces).mockImplementation((image) => ({ promise: detectFaces(image), release: vi.fn() }));
  vi.spyOn(Element.prototype, 'clientWidth', 'get').mockReturnValue(600);
  vi.spyOn(Element.prototype, 'clientHeight', 'get').mockReturnValue(500);
  let correction!: (action: ViewerCorrectionAction) => void;
  let publish!: (value: ViewerPresentation) => void;
  let refresh!: () => void;
  let activeSessionId = '';
  let resolveOld!: (value: { token: string; url: string }) => void;
  const oldPreparation = new Promise<{ token: string; url: string }>((resolve) => { resolveOld = resolve; });
  const prepare = vi.fn(async (_request: Parameters<NonNullable<ImageViewerWindowPort['prepareCorrection']>>[0]) => ({ token: 'new', url: 'data:image/png;id=result-new' }))
    .mockImplementationOnce(() => oldPreparation);
  const port: ImageViewerWindowPort = {
    open: async value => publish(value), update: async value => publish(value), close: async () => {},
    onNavigate: async () => () => {}, onClosed: async () => () => {},
    onCorrection: async value => { correction = value; return () => {}; }, prepareCorrection: prepare,
    cancelCorrection: async () => {},
  };
  const mutation = {} as ProjectMutationRunner;
  const projectionChanged = vi.fn();
  const basePhoto = representativeProjection.state.album.media.find(item => item.kind === 'photo')!;
  const media = [{ ...basePhoto, id: 'a', name: 'Target' }, { ...basePhoto, id: 'b', name: 'Reference' }];
  const previews = { a: { mediaId: 'a', state: 'ready' as const, url: 'data:image/png;id=a' }, b: { mediaId: 'b', state: 'ready' as const, url: 'data:image/png;id=b' } };
  function Harness() {
    const [presentation, setPresentation] = useState<ViewerPresentation | null>(null);
    const [version, setVersion] = useState(0);
    publish = value => { activeSessionId = value.sessionId; setPresentation(value); };
    refresh = () => setVersion(value => value + 1);
    const previewUrls = useMemo(() => ({ a: `data:image/png;id=a-${side === 'target' ? version : 0}`,
      b: `data:image/png;id=b-${side === 'reference' ? version : 0}` }), [version]);
    const session = useImageViewerSession({ projectId: 'p', media, previews,
      previewUrls,
      port, mutation, onProjectionChange: projectionChanged });
    return <><button onClick={event => session.open('panel', 'a', ['a', 'b'], event.currentTarget)}>Open</button>
      {presentation && <ImageViewer presentation={presentation} onNavigate={() => {}} onClose={() => {}} onCorrection={action => correction(action)} />}</>;
  }
  render(<Harness />);
  fireEvent.click(screen.getByRole('button', { name: 'Open' }));
  const load = (image: HTMLElement) => {
    Object.defineProperties(image, { naturalWidth: { configurable: true, value: 800 }, naturalHeight: { configurable: true, value: 1200 } });
    fireEvent.load(image);
  };
  load(await screen.findByRole('img', { name: 'Target' }));
  fireEvent.click(screen.getByRole('button', { name: 'Abrir olhos' }));
  for (const image of screen.getAllByRole('img')) load(image);
  await waitFor(() => expect(screen.getByRole('button', { name: 'Usar esta foto' })).not.toHaveAttribute('aria-disabled', 'true'));
  fireEvent.click(screen.getByRole('button', { name: 'Usar esta foto' }));
  load(screen.getByRole('img', { name: 'Reference' }));
  fireEvent.click(await screen.findByRole('button', { name: 'Imagem a corrigir: rosto 1' }));
  fireEvent.click(await screen.findByRole('button', { name: 'Referência: rosto 1' }));
  await waitFor(() => expect(prepare).toHaveBeenCalledTimes(1));
  const detectionBeforeRefresh = detectFaces.mock.calls.length;
  const newFace = [{ x: .1, y: .1, z: 0 }, { x: .2, y: .2, z: 0 }];
  detectFaces.mockResolvedValue([newFace]);
  act(() => refresh());
  expect(prepare).toHaveBeenCalledTimes(1);
  expect(screen.queryByRole('button', { name: 'Salvar correção' })).not.toBeInTheDocument();
  correction({ sessionId: activeSessionId, kind: 'preview', referenceMediaId: 'b',
    targetUrl: 'data:image/png;id=a-0', referenceUrl: 'data:image/png;id=b-0', targetFace: oldFace, referenceFace: oldFace });
  expect(prepare).toHaveBeenCalledTimes(1);
  load(screen.getByRole('img', { name: side === 'reference' ? 'Reference' : 'Target' }));
  await waitFor(() => expect(detectFaces.mock.calls.length).toBe(detectionBeforeRefresh + 1));
  const updatedFace = await screen.findByRole('button', { name: side === 'reference' ? 'Referência: rosto 1' : 'Imagem a corrigir: rosto 1' });
  expect(updatedFace).toHaveAttribute('aria-pressed', 'false');
  await act(async () => resolveOld({ token: 'old', url: 'data:image/png;id=result-old' }));
  expect(screen.queryByRole('button', { name: 'Salvar correção' })).not.toBeInTheDocument();
  fireEvent.click(updatedFace);
  await waitFor(() => expect(prepare).toHaveBeenCalledTimes(2));
  expect(prepare.mock.calls[1][0]).toEqual(expect.objectContaining(side === 'reference' ? { referenceFace: newFace } : { targetFace: newFace }));
  await screen.findByRole('button', { name: 'Salvar correção' });
  act(() => refresh());
  await waitFor(() => expect(screen.queryByRole('button', { name: 'Salvar correção' })).not.toBeInTheDocument());
  expect(prepare).toHaveBeenCalledTimes(2);
});
