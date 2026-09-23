import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { expect, test, vi } from 'vitest';
import { ImageViewer } from './ImageViewer';
import { acquireFaces, type Face } from '../image-viewer/faceLandmarks';
vi.mock('../image-viewer/faceLandmarks', async original => ({ ...(await original()), acquireFaces: vi.fn() }));
const detectFaces = vi.fn<(image: HTMLImageElement) => Promise<Face[]>>();

test('a selected pair requests preview once while its work is pending', async () => {
  detectFaces.mockResolvedValue([[{ x: .4, y: .3, z: 0 }, { x: .6, y: .5, z: 0 }]]);
  vi.mocked(acquireFaces).mockImplementation((image) => ({ promise: detectFaces(image), release: vi.fn() }));
  vi.spyOn(Element.prototype, 'clientWidth', 'get').mockReturnValue(600);
  vi.spyOn(Element.prototype, 'clientHeight', 'get').mockReturnValue(500);
  const onCorrection = vi.fn();
  const base = { sessionId: 'review', revision: 1, mediaId: 'a', name: 'Alvo', url: 'data:image/png;id=a', state: 'ready' as const, canPrevious: false, canNext: true };
  const correction = { phase: 'select' as const, referenceMediaId: 'b', referenceName: 'Ref', referenceUrl: 'data:image/png;id=b', referenceState: 'ready' as const, canPreviousReference: false, canNextReference: false, resultUrl: null, error: null };
  const view = render(<ImageViewer presentation={{ ...base, correction }} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={onCorrection} />);
  for (const image of screen.getAllByRole('img')) {
    Object.defineProperties(image, { naturalWidth: { configurable: true, value: 800 }, naturalHeight: { configurable: true, value: 1200 } });
    fireEvent.load(image);
  }
  fireEvent.click(await screen.findByRole('button', { name: 'Imagem a corrigir: rosto 1' }));
  fireEvent.click(screen.getByRole('button', { name: 'Referência: rosto 1' }));
  await waitFor(() => expect(onCorrection).toHaveBeenCalledTimes(1));
  fireEvent.click(screen.getByRole('button', { name: 'Imagem a corrigir: rosto 1' }));
  fireEvent.click(screen.getByRole('button', { name: 'Referência: rosto 1' }));
  expect(onCorrection).toHaveBeenCalledTimes(1);
  view.rerender(<ImageViewer presentation={{ ...base, revision: 2, correction: { ...correction, phase: 'preview', resultUrl: 'data:image/png;id=result' } }} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={onCorrection} />);
  expect(screen.getByRole('button', { name: 'Salvar correção' })).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: 'Imagem a corrigir: rosto 1' }));
  expect(onCorrection).toHaveBeenCalledTimes(1);
  expect(screen.getByRole('button', { name: 'Salvar correção' })).toBeInTheDocument();
  view.rerender(<ImageViewer presentation={{ ...base, revision: 3, correction: { ...correction, phase: 'preview', resultUrl: 'data:image/png;id=result', error: 'Falha ao salvar.' } }} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={onCorrection} />);
  fireEvent.click(screen.getByRole('button', { name: 'Imagem a corrigir: rosto 1' }));
  expect(onCorrection).toHaveBeenCalledTimes(1);
  expect(screen.getByRole('button', { name: 'Salvar correção' })).toBeInTheDocument();
  view.rerender(<ImageViewer presentation={{ ...base, revision: 4, correction: { ...correction, error: 'Falha ao preparar.' } }} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={onCorrection} />);
  fireEvent.click(screen.getByRole('button', { name: 'Imagem a corrigir: rosto 1' }));
  await waitFor(() => expect(onCorrection).toHaveBeenCalledTimes(2));
});

test('changing reference releases its analysis while target analysis stays acquired', () => {
  const releases = new Map<string, ReturnType<typeof vi.fn>>();
  vi.mocked(acquireFaces).mockImplementation((image) => {
    const release = vi.fn();
    releases.set(image.src, release);
    return { promise: new Promise<Face[]>(() => undefined), release };
  });
  const base = { sessionId: 'lease', revision: 1, mediaId: 'a', name: 'Target', url: 'data:image/png;id=a', state: 'ready' as const, canPrevious: false, canNext: false };
  const correction = { phase: 'select' as const, referenceMediaId: 'b', referenceName: 'Reference', referenceUrl: 'data:image/png;id=b',
    referenceState: 'ready' as const, canPreviousReference: false, canNextReference: false, resultUrl: null, error: null };
  const props = { onNavigate: vi.fn(), onClose: vi.fn(), onCorrection: vi.fn() };
  const view = render(<ImageViewer presentation={{ ...base, correction }} {...props} />);
  const load = (image: HTMLElement) => {
    Object.defineProperties(image, { naturalWidth: { configurable: true, value: 800 }, naturalHeight: { configurable: true, value: 600 } });
    fireEvent.load(image);
  };
  for (const image of screen.getAllByRole('img')) load(image);
  expect(releases.has('data:image/png;id=a')).toBe(true);
  expect(releases.has('data:image/png;id=b')).toBe(true);
  view.rerender(<ImageViewer presentation={{ ...base, revision: 2, correction: { ...correction, referenceMediaId: 'c', referenceUrl: 'data:image/png;id=c' } }} {...props} />);
  expect(releases.get('data:image/png;id=b')).toHaveBeenCalledOnce();
  expect(releases.get('data:image/png;id=a')).not.toHaveBeenCalled();
  load(screen.getByRole('img', { name: 'Reference' }));
  expect(releases.has('data:image/png;id=c')).toBe(true);
  view.rerender(<ImageViewer presentation={{ ...base, revision: 3, correction: { ...correction, phase: 'browse', referenceMediaId: 'c', referenceUrl: 'data:image/png;id=c' } }} {...props} />);
  expect(releases.get('data:image/png;id=c')).toHaveBeenCalledOnce();
  expect(releases.get('data:image/png;id=a')).not.toHaveBeenCalled();
});
