"""Makes the two PDF areas in a QualCoder project the way QualCoder itself holds them (points from the TOP left of the page), for the same
places PrismQDA drew them: page 1 at 0.1/0.12 of a 612 x 792 page, 0.8 wide and 0.1 tall; page 2 at 0.2/0.3, 0.5 wide and 0.2 tall."""
import sqlite3
import sys

con = sqlite3.connect(sys.argv[1] + "/data.qda")
cur = con.cursor()
cur.execute("update code_image set x1=61, y1=95, width=490, height=79 where pdf_page=0")
cur.execute("update code_image set x1=122, y1=238, width=306, height=158 where pdf_page=1")
con.commit()
cur.execute("select imid, pdf_page, x1, y1, width, height from code_image where pdf_page is not null")
print("native PDF areas:", cur.fetchall())
