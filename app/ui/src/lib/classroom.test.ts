import { test } from 'node:test';
import assert from 'node:assert/strict';
import { heatLevel, kappaText, parseStudents, percentText, suggestedClassFolder, suggestedClassTable, suggestedReviewFile } from './classroom';

test('the heat map is shaded darker the fewer of the instructor\'s passages a student found, and not at all when all were', () => {
  assert.deepEqual([1, 0.9, 0.75, 0.6, 0.5, 0.3, 0.25, 0.1, 0].map(heatLevel), [0, 1, 1, 2, 2, 3, 3, 4, 4]);
  assert.equal(heatLevel(null), 0, 'nothing to divide is not shaded');
  assert.equal(heatLevel(undefined), 0);
  assert.equal(heatLevel(Number.NaN), 0);
});

test('the file for a table of the class is named for the project, with no character Windows refuses', () => {
  assert.equal(suggestedClassTable('Riders: 2026', 'class heat map'), 'Riders- 2026 class heat map.csv');
  assert.match(suggestedClassTable('', 'class side by side'), /class side by side\.csv$/);
});

test('a pasted list of students is one name to a line, tidied, with a repeated name kept once', () => {
  const { names, repeated } = parseStudents('  Annika   Alder \r\n\r\nBohdan Brandt\n   \nannika alder\nCyrus');
  assert.deepEqual(names, ['Annika Alder', 'Bohdan Brandt', 'Cyrus']);
  assert.deepEqual(repeated, ['annika alder']);
  assert.deepEqual(parseStudents(''), { names: [], repeated: [] });
});

test('the folder for a class is beside the project and named for it', () => {
  assert.equal(suggestedClassFolder('C:\\Courses\\Intro\\Exercise 1.pqda'), 'C:\\Courses\\Intro\\Exercise 1 class packets');
  assert.equal(suggestedClassFolder('/home/me/Exercise.pqda'), '/home/me/Exercise class packets');
  assert.equal(suggestedClassFolder('Exercise.pqda'), 'Exercise class packets');
});

test('a share is a whole percent and nothing to divide is a dash', () => {
  assert.equal(percentText(0.5), '50%');
  assert.equal(percentText(1 / 3), '33%');
  assert.equal(percentText(0), '0%');
  assert.equal(percentText(null), '–');
  assert.equal(percentText(Number.NaN), '–');
});

test('a kappa has two places, and an undefined one is said', () => {
  assert.equal(kappaText(0.5454545), '0.55');
  assert.equal(kappaText(-0.1), '-0.10');
  assert.equal(kappaText(null), 'not defined');
});

test('the file for the table of students is named for the project, with no character Windows refuses', () => {
  assert.equal(suggestedReviewFile('Intro: week 3'), 'Intro- week 3 class review.csv');
  assert.equal(suggestedReviewFile('  '), 'Class class review.csv');
});
