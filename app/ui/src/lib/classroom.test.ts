import { test } from 'node:test';
import assert from 'node:assert/strict';
import { kappaText, parseStudents, percentText, suggestedClassFolder, suggestedReviewFile } from './classroom';

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
