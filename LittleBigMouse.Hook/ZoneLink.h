#pragma once

class Zone;

class ZoneLink
{
public:

	Zone* Target;
	long TargetId;
	ZoneLink* Next;
	double From;
	double To;
	long SourceFromPixel;
	long SourceToPixel;
	long TargetFromPixel;
	long TargetToPixel;
	//long SourceLengthPixel;
	//long TargetLengthPixel;
	Zone* At(const double pos) const;
	const ZoneLink* AtPixel(long pos) const;
	long ToTargetPixel(long v) const;
	ZoneLink(double from, double to, long sourceFromPixel, long sourceToPixel, long targetFromPixel, long targetToPixel, long targetId = -1);
	~ZoneLink() {delete Next;}
};

