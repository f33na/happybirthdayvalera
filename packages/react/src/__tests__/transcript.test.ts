import { describe, expect, it } from 'vitest';
import { cleanTranscript } from '../core/transcript';

describe('cleanTranscript', () => {
  it.each([
    ['[BLANK_AUDIO]', ''],
    [' [Music] ', ''],
    ['(смеется)', ''],
    ['*applause*', ''],
    ['♪', ''],
    ['♪ ♫ ♪', ''],
    ['[MUSIC PLAYING] (applause) ...', ''],
    ['', ''],
    ['Субтитры сделал DimaTorzok', ''],
    ['Субтитры сделал DimaTorzok.', ''],
    [' Thank you. ', ''],
    ['Thanks for watching!', ''],
    ['Продолжение следует...', ''],
    ['Merci d’avoir regardé !', ''],
    ['[BLANK_AUDIO] Thank you for watching!', ''],
    ['[BLANK_AUDIO] привет', 'привет'],
    ['Hello [Music] world ♪', 'Hello world'],
    ['Thank you for the flowers, Valera.', 'Thank you for the flowers, Valera.'],
    ['Я сказал: спасибо за просмотр, и ушёл.', 'Я сказал: спасибо за просмотр, и ушёл.'],
    ['  Happy   birthday \n Valera ', 'Happy birthday Valera'],
    ['Room 42', 'Room 42'],
  ])('%j → %j', (input, expected) => {
    expect(cleanTranscript(input)).toBe(expected);
  });
});
