import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { expect, test, vi } from 'vitest';
import { ImageViewer } from './ImageViewer';
import { detectFaces } from '../image-viewer/faceLandmarks';
vi.mock('../image-viewer/faceLandmarks', async original => ({ ...(await original()), detectFaces: vi.fn() }));

test('a selected pair requests preview once while its work is pending', async () => {
  vi.mocked(detectFaces).mockResolvedValue([[{ x: .4, y: .3, z: 0 }, { x: .6, y: .5, z: 0 }]]);
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
